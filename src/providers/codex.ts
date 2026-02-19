import { join } from 'path';
import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { ModelWindows, Provider, ProviderQuota, QuotaWindow } from './types';

interface CodexWindowRaw {
  used_percent: number;
  window_minutes: number;
  resets_at: number;
}

interface CodexLimitBucket {
  limit_id: string;
  limit_name?: string | null;
  primary?: CodexWindowRaw;
  secondary?: CodexWindowRaw;
}

interface CodexRateLimits {
  primary?: CodexWindowRaw;
  secondary?: CodexWindowRaw;
  credits?: {
    has_credits: boolean;
    unlimited: boolean;
    balance: string;
  };
  plan_type?: string | null;
  buckets?: Record<string, CodexLimitBucket>;
}

interface CodexAppServerWindow {
  usedPercent: number;
  windowDurationMins?: number | null;
  resetsAt?: number | null;
}

interface CodexAppServerLimitBucket {
  limitId?: string | null;
  limitName?: string | null;
  primary?: CodexAppServerWindow | null;
  secondary?: CodexAppServerWindow | null;
  planType?: string | null;
}

interface CodexAppServerRateLimitsReadResult {
  rateLimits?: CodexAppServerLimitBucket | null;
  rateLimitsByLimitId?: Record<string, CodexAppServerLimitBucket> | null;
  credits?: { hasCredits: boolean; unlimited: boolean; balance?: string | null } | null;
  planType?: string | null;
}

interface CodexAppServerAccountReadResult {
  account?: {
    planType?: string | null;
  } | null;
}

interface CodexSessionEvent {
  payload?: {
    type?: string;
    rate_limits?: CodexRateLimits;
  };
}

export class CodexProvider implements Provider {
  readonly id = 'codex';
  readonly name = 'Codex';

  async isAvailable(): Promise<boolean> {
    const file = Bun.file(CONFIG.paths.codex.auth);
    return await file.exists();
  }

  private async findLatestSessionFile(): Promise<string | null> {
    const sessionsDir = CONFIG.paths.codex.sessions;
    const now = new Date();

    // Check today and yesterday.
    for (let dayOffset = 0; dayOffset < 2; dayOffset++) {
      const date = new Date(now);
      date.setDate(date.getDate() - dayOffset);

      const year = date.getFullYear().toString().padStart(4, '0');
      const month = (date.getMonth() + 1).toString().padStart(2, '0');
      const day = date.getDate().toString().padStart(2, '0');

      const dayDir = join(sessionsDir, year, month, day);

      try {
        const glob = new Bun.Glob('*.jsonl');
        const files: string[] = [];

        for await (const file of glob.scan({ cwd: dayDir, absolute: true })) {
          files.push(file);
        }

        if (files.length > 0) {
          const sorted = await Promise.all(
            files.map(async (f) => ({
              path: f,
              mtime: (await Bun.file(f).stat()).mtimeMs,
            }))
          );
          sorted.sort((a, b) => b.mtime - a.mtime);
          return sorted[0].path;
        }
      } catch {
        continue;
      }
    }

    return null;
  }

  private async extractRateLimits(filePath: string): Promise<CodexRateLimits | null> {
    try {
      const content = await Bun.file(filePath).text();
      const lines = content.trim().split('\n').reverse();

      for (const line of lines) {
        if (!line.trim()) continue;

        try {
          const event: CodexSessionEvent = JSON.parse(line);
          if (event.payload?.type === 'token_count' && event.payload.rate_limits) {
            return event.payload.rate_limits;
          }
        } catch {
          continue;
        }
      }
    } catch (error) {
      logger.error('Failed to read Codex session file', { error, filePath });
    }

    return null;
  }

  private unixToIso(timestamp: number): string | null {
    if (!timestamp || timestamp <= 0) return null;
    return new Date(timestamp * 1000).toISOString();
  }

  private normalizePlanName(planType: string | null | undefined): string | undefined {
    if (!planType) return undefined;
    const key = planType.trim().toLowerCase();
    const map: Record<string, string> = {
      free: 'Free',
      go: 'Go',
      plus: 'Plus',
      pro: 'Pro',
      business: 'Business',
      team: 'Business',
      enterprise: 'Enterprise',
      edu: 'Edu',
      education: 'Edu',
      apikey: 'API Key',
      api_key: 'API Key',
    };
    return map[key] ?? planType;
  }

  private toRawWindow(
    raw: CodexAppServerWindow | null | undefined,
    fallbackMinutes: number
  ): CodexWindowRaw | undefined {
    if (!raw) return undefined;
    return {
      used_percent: raw.usedPercent,
      window_minutes: (raw.windowDurationMins ?? fallbackMinutes) as number,
      resets_at: (raw.resetsAt ?? 0) as number,
    };
  }

  private normalizeBucket(
    raw: CodexAppServerLimitBucket,
    fallbackId?: string
  ): CodexLimitBucket | null {
    const limitId = raw.limitId ?? fallbackId ?? null;
    if (!limitId) return null;

    const primary = this.toRawWindow(raw.primary, 300);
    const secondary = this.toRawWindow(raw.secondary, 10080);
    if (!primary && !secondary) return null;

    return {
      limit_id: limitId,
      limit_name: raw.limitName ?? null,
      ...(primary ? { primary } : {}),
      ...(secondary ? { secondary } : {}),
    };
  }

  private normalizeAppServerRateLimits(
    raw: CodexAppServerRateLimitsReadResult,
    accountPlanType?: string | null
  ): CodexRateLimits | null {
    const buckets: Record<string, CodexLimitBucket> = {};

    if (raw.rateLimitsByLimitId) {
      for (const [limitId, bucket] of Object.entries(raw.rateLimitsByLimitId)) {
        const normalized = this.normalizeBucket(bucket, limitId);
        if (normalized) {
          buckets[normalized.limit_id] = normalized;
        }
      }
    }

    const root = raw.rateLimits ?? null;
    const rootBucket = root ? this.normalizeBucket(root, root.limitId ?? 'codex') : null;
    if (rootBucket && !buckets[rootBucket.limit_id]) {
      buckets[rootBucket.limit_id] = rootBucket;
    }

    const firstBucket = Object.values(buckets)[0];
    const primary = rootBucket?.primary ?? firstBucket?.primary;
    const secondary = rootBucket?.secondary ?? firstBucket?.secondary;

    if (!primary && !secondary && Object.keys(buckets).length === 0) {
      return null;
    }

    const credits = raw.credits
      ? {
          has_credits: raw.credits.hasCredits,
          unlimited: raw.credits.unlimited,
          balance: raw.credits.balance ?? '0',
        }
      : undefined;

    return {
      ...(primary ? { primary } : {}),
      ...(secondary ? { secondary } : {}),
      ...(Object.keys(buckets).length > 0 ? { buckets } : {}),
      ...(credits ? { credits } : {}),
      plan_type: accountPlanType ?? raw.planType ?? root?.planType ?? null,
    };
  }

  private classifyWindow(minutes: number | null | undefined): 'fiveHour' | 'sevenDay' | 'other' {
    if (!minutes || minutes <= 0) return 'other';

    // Keep classification tolerant to provider-side changes.
    if (Math.abs(minutes - 300) <= 90) return 'fiveHour';
    if (Math.abs(minutes - 10080) <= 1440) return 'sevenDay';
    return 'other';
  }

  private toQuotaWindow(raw: CodexWindowRaw | undefined): QuotaWindow | undefined {
    if (!raw) return undefined;
    return {
      remaining: 100 - Math.round(raw.used_percent),
      resetsAt: this.unixToIso(raw.resets_at),
      windowMinutes: raw.window_minutes ?? null,
    };
  }

  private formatBucketLabel(bucket: CodexLimitBucket): string {
    const raw = (bucket.limit_name && bucket.limit_name.trim().length > 0)
      ? bucket.limit_name
      : bucket.limit_id;
    const normalized = raw.replace(/[_-]+/g, ' ').trim();
    if (!normalized) return 'Codex';
    return normalized
      .split(/\s+/)
      .map((part) => part ? part[0].toUpperCase() + part.slice(1) : part)
      .join(' ');
  }

  private buildModelWindows(limits: CodexRateLimits): Record<string, ModelWindows> {
    const modelsDetailed: Record<string, ModelWindows> = {};

    if (limits.buckets && Object.keys(limits.buckets).length > 0) {
      for (const bucket of Object.values(limits.buckets)) {
        const windows: ModelWindows = {};
        const all = [bucket.primary, bucket.secondary].filter(Boolean) as CodexWindowRaw[];

        for (const raw of all) {
          const quotaWindow = this.toQuotaWindow(raw);
          if (!quotaWindow) continue;

          const kind = this.classifyWindow(raw.window_minutes);
          if (kind === 'fiveHour' && !windows.fiveHour) {
            windows.fiveHour = quotaWindow;
          } else if (kind === 'sevenDay' && !windows.sevenDay) {
            windows.sevenDay = quotaWindow;
          } else {
            if (!windows.other) windows.other = [];
            windows.other.push(quotaWindow);
          }
        }

        // Fallback mapping when durations don't classify cleanly.
        if (!windows.fiveHour && bucket.primary) {
          windows.fiveHour = this.toQuotaWindow(bucket.primary);
        }
        if (!windows.sevenDay && bucket.secondary) {
          windows.sevenDay = this.toQuotaWindow(bucket.secondary);
        }

        if (!windows.fiveHour && !windows.sevenDay && (!windows.other || windows.other.length === 0)) {
          continue;
        }

        const baseName = this.formatBucketLabel(bucket);
        let name = baseName;
        let suffix = 2;
        while (modelsDetailed[name]) {
          name = `${baseName} (${suffix++})`;
        }
        modelsDetailed[name] = windows;
      }
    }

    // Legacy fallback when only primary/secondary are available.
    if (Object.keys(modelsDetailed).length === 0 && (limits.primary || limits.secondary)) {
      const windows: ModelWindows = {};
      for (const raw of [limits.primary, limits.secondary]) {
        if (!raw) continue;
        const quotaWindow = this.toQuotaWindow(raw);
        if (!quotaWindow) continue;
        const kind = this.classifyWindow(raw.window_minutes);
        if (kind === 'fiveHour' && !windows.fiveHour) windows.fiveHour = quotaWindow;
        else if (kind === 'sevenDay' && !windows.sevenDay) windows.sevenDay = quotaWindow;
        else {
          if (!windows.other) windows.other = [];
          windows.other.push(quotaWindow);
        }
      }
      if (!windows.fiveHour && limits.primary) {
        windows.fiveHour = this.toQuotaWindow(limits.primary);
      }
      if (!windows.sevenDay && limits.secondary) {
        windows.sevenDay = this.toQuotaWindow(limits.secondary);
      }
      modelsDetailed['Codex'] = windows;
    }

    return modelsDetailed;
  }

  private flattenModels(modelsDetailed: Record<string, ModelWindows>): Record<string, QuotaWindow> {
    const models: Record<string, QuotaWindow> = {};
    for (const [name, windows] of Object.entries(modelsDetailed)) {
      const selected = windows.fiveHour ?? windows.sevenDay ?? windows.other?.[0];
      if (selected) models[name] = selected;
    }
    return models;
  }

  private pickPrimary(
    limits: CodexRateLimits,
    modelsDetailed: Record<string, ModelWindows>
  ): QuotaWindow | undefined {
    const explicit = this.toQuotaWindow(limits.primary);
    if (explicit) return explicit;

    for (const model of Object.values(modelsDetailed)) {
      if (model.fiveHour) return model.fiveHour;
    }
    for (const model of Object.values(modelsDetailed)) {
      if (model.sevenDay) return model.sevenDay;
    }
    return undefined;
  }

  private pickSecondary(
    limits: CodexRateLimits,
    modelsDetailed: Record<string, ModelWindows>
  ): QuotaWindow | undefined {
    const explicit = this.toQuotaWindow(limits.secondary);
    if (explicit) return explicit;

    for (const model of Object.values(modelsDetailed)) {
      if (model.sevenDay) return model.sevenDay;
    }
    return undefined;
  }

  private async fetchRateLimitsViaAppServer(timeoutMs: number = 4000): Promise<CodexRateLimits | null> {
    // Codex app-server exposes a stable JSON-RPC-ish protocol over stdio.
    const { spawn } = await import('node:child_process');
    const { createInterface } = await import('node:readline');

    return await new Promise<CodexRateLimits | null>((resolve) => {
      const proc = spawn('codex', ['app-server'], {
        stdio: ['pipe', 'pipe', 'ignore'],
      });

      const rl = createInterface({ input: proc.stdout });

      let finished = false;
      let accountPlanType: string | null | undefined = undefined;
      let rateLimitsResult: CodexAppServerRateLimitsReadResult | null = null;
      let graceTimer: ReturnType<typeof setTimeout> | null = null;

      const cleanup = (result: CodexRateLimits | null) => {
        if (finished) return;
        finished = true;
        if (graceTimer) clearTimeout(graceTimer);
        clearTimeout(timer);
        try { rl.close(); } catch {}
        try { proc.kill(); } catch {}
        resolve(result);
      };

      const tryResolve = () => {
        if (!rateLimitsResult) return;
        const normalized = this.normalizeAppServerRateLimits(rateLimitsResult, accountPlanType);
        cleanup(normalized);
      };

      const timer = setTimeout(() => cleanup(null), timeoutMs);

      const send = (msg: unknown) => {
        try {
          proc.stdin.write(JSON.stringify(msg) + '\n');
        } catch {
          // ignore
        }
      };

      proc.on('error', () => cleanup(null));
      proc.on('exit', () => cleanup(null));

      rl.on('line', (line: string) => {
        try {
          const msg = JSON.parse(line) as any;

          if (msg?.id === 0 && msg?.result) {
            send({ method: 'initialized', params: {} });
            send({ method: 'account/read', id: 1, params: { refreshToken: false } });
            send({ method: 'account/rateLimits/read', id: 2, params: {} });
            return;
          }

          if (msg?.id === 1 && msg?.result) {
            const accountResult = msg.result as CodexAppServerAccountReadResult;
            accountPlanType = accountResult.account?.planType ?? null;
            if (rateLimitsResult) tryResolve();
            return;
          }

          if (msg?.id === 2 && msg?.result && (msg.result.rateLimits || msg.result.rateLimitsByLimitId)) {
            rateLimitsResult = msg.result as CodexAppServerRateLimitsReadResult;
            if (accountPlanType !== undefined) {
              tryResolve();
              return;
            }

            // account/read can arrive slightly after limits; wait a short grace period.
            if (graceTimer) clearTimeout(graceTimer);
            graceTimer = setTimeout(() => {
              if (!finished) tryResolve();
            }, 200);
          }
        } catch {
          // ignore non-json / unrelated messages
        }
      });

      send({
        method: 'initialize',
        id: 0,
        params: {
          clientInfo: { name: 'qbar', title: 'qbar', version: '3.0.0' },
        },
      });
    });
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    if (!await this.isAvailable()) {
      return { ...base, error: 'Not logged in' };
    }

    const cached = await cache.get<CodexRateLimits>('codex-quota');
    let limits = cached;

    if (!limits) {
      limits = await this.fetchRateLimitsViaAppServer();

      if (!limits) {
        const sessionFile = await this.findLatestSessionFile();
        if (!sessionFile) {
          return { ...base, error: 'No session data found' };
        }

        limits = await this.extractRateLimits(sessionFile);
        if (!limits) {
          return { ...base, error: 'No rate limit data found (app-server + session log)' };
        }
      }

      await cache.set('codex-quota', limits, CONFIG.cache.codexTtlMs);
    }

    const modelsDetailed = this.buildModelWindows(limits);
    const models = this.flattenModels(modelsDetailed);
    const primary = this.pickPrimary(limits, modelsDetailed);
    const secondary = this.pickSecondary(limits, modelsDetailed);

    if (!primary && !secondary && Object.keys(modelsDetailed).length === 0) {
      return { ...base, error: 'No quota windows found' };
    }

    let codexCredits: ProviderQuota['extraUsage'] | undefined;
    if (limits.credits?.has_credits || parseFloat(limits.credits?.balance || '0') > 0) {
      const balance = parseFloat(limits.credits!.balance);
      codexCredits = {
        enabled: true,
        remaining: limits.credits!.unlimited ? 100 : Math.min(100, Math.round(balance)),
        limit: limits.credits!.unlimited ? -1 : 0,
        used: 0,
      };
    }

    return {
      ...base,
      available: true,
      ...(primary ? { primary } : {}),
      ...(secondary ? { secondary } : {}),
      ...(Object.keys(models).length > 0 ? { models } : {}),
      ...(Object.keys(modelsDetailed).length > 0 ? { modelsDetailed } : {}),
      ...(limits.plan_type ? { planType: limits.plan_type } : {}),
      ...(this.normalizePlanName(limits.plan_type) ? { plan: this.normalizePlanName(limits.plan_type) } : {}),
      ...(codexCredits ? { extraUsage: codexCredits } : {}),
    };
  }
}

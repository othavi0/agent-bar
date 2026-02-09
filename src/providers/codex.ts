import { join } from 'path';
import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

interface CodexRateLimits {
  primary: {
    used_percent: number;
    window_minutes: number;
    resets_at: number;
  };
  secondary?: {
    used_percent: number;
    window_minutes: number;
    resets_at: number;
  };
  credits?: {
    has_credits: boolean;
    unlimited: boolean;
    balance: string;
  };
  plan_type?: string | null;
}

interface CodexAppServerRateLimits {
  primary?: { usedPercent: number; windowDurationMins?: number | null; resetsAt?: number | null } | null;
  secondary?: { usedPercent: number; windowDurationMins?: number | null; resetsAt?: number | null } | null;
  credits?: { hasCredits: boolean; unlimited: boolean; balance?: string | null } | null;
  planType?: string | null;
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

    // Check today and yesterday
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
          // Return most recently modified
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
        // Directory doesn't exist or error reading
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

  private normalizeAppServerRateLimits(raw: CodexAppServerRateLimits): CodexRateLimits | null {
    if (!raw.primary) return null;

    // app-server uses usedPercent (0-100 consumed). Keep our internal shape consistent.
    const credits = raw.credits
      ? {
          has_credits: raw.credits.hasCredits,
          unlimited: raw.credits.unlimited,
          balance: raw.credits.balance ?? '0',
        }
      : undefined;

    const primary = {
      used_percent: raw.primary.usedPercent,
      window_minutes: (raw.primary.windowDurationMins ?? 300) as number,
      resets_at: (raw.primary.resetsAt ?? 0) as number,
    };

    const secondary = raw.secondary
      ? {
          used_percent: raw.secondary.usedPercent,
          window_minutes: (raw.secondary.windowDurationMins ?? 10080) as number,
          resets_at: (raw.secondary.resetsAt ?? 0) as number,
        }
      : undefined;

    return {
      primary,
      secondary,
      credits,
      plan_type: raw.planType ?? null,
    };
  }

  private async fetchRateLimitsViaAppServer(timeoutMs: number = 4000): Promise<CodexRateLimits | null> {
    // Codex app-server exposes a stable JSON-RPC-ish protocol over stdio.
    // We only need account/rateLimits/read.
    const { spawn } = await import('node:child_process');
    const { createInterface } = await import('node:readline');

    return await new Promise<CodexRateLimits | null>((resolve) => {
      // Ignore stderr to avoid backpressure if Codex writes logs there.
      const proc = spawn('codex', ['app-server'], {
        stdio: ['pipe', 'pipe', 'ignore'],
      });

      const rl = createInterface({ input: proc.stdout });

      let finished = false;
      const cleanup = (result: CodexRateLimits | null) => {
        if (finished) return;
        finished = true;
        try { rl.close(); } catch {}
        try { proc.kill(); } catch {}
        resolve(result);
      };

      const timer = setTimeout(() => cleanup(null), timeoutMs);

      const send = (msg: unknown) => {
        try {
          proc.stdin.write(JSON.stringify(msg) + '\n');
        } catch {
          // ignore
        }
      };

      proc.on('error', () => {
        clearTimeout(timer);
        cleanup(null);
      });

      proc.on('exit', () => {
        clearTimeout(timer);
        cleanup(null);
      });

      rl.on('line', (line: string) => {
        try {
          const msg = JSON.parse(line) as any;
          if (msg?.id === 0 && msg?.result) {
            // init ack
            send({ method: 'initialized', params: {} });
            send({ method: 'account/rateLimits/read', id: 1, params: {} });
            return;
          }

          if (msg?.id === 1 && msg?.result?.rateLimits) {
            clearTimeout(timer);
            const normalized = this.normalizeAppServerRateLimits(msg.result.rateLimits as CodexAppServerRateLimits);
            cleanup(normalized);
          }
        } catch {
          // ignore non-json
        }
      });

      // Kick off handshake.
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

    // Check if logged in
    if (!await this.isAvailable()) {
      return { ...base, error: 'Not logged in' };
    }

    // Try to get cached data first
    const cached = await cache.get<CodexRateLimits>('codex-quota');
    let limits = cached;

    if (!limits) {
      // Prefer the official app-server endpoint (stable, structured, does not depend on session JSONL).
      limits = await this.fetchRateLimitsViaAppServer();

      // Fallback: try to parse the latest session file (legacy behavior).
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

      // Cache the result
      await cache.set('codex-quota', limits, CONFIG.cache.codexTtlMs);
    }

    // Build quota response
    const primary: QuotaWindow = {
      remaining: 100 - Math.round(limits.primary.used_percent),
      resetsAt: this.unixToIso(limits.primary.resets_at),
      windowMinutes: limits.primary.window_minutes ?? null,
    };

    const secondary: QuotaWindow | undefined = limits.secondary
      ? {
          remaining: 100 - Math.round(limits.secondary.used_percent),
          resetsAt: this.unixToIso(limits.secondary.resets_at),
          windowMinutes: limits.secondary.window_minutes ?? null,
        }
      : undefined;

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
      primary,
      ...(secondary ? { secondary } : {}),
      extraUsage: codexCredits,
    };
  }
}

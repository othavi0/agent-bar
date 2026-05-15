import { Buffer } from 'node:buffer';
import { APP_NAME } from '../app-identity';
import { CONFIG } from '../config';
import { COPILOT_MISSING_ERROR, findCopilotBin } from '../copilot-cli';
import type { QuotaBase } from './base';
import { BaseProvider } from './base';
import { registerProvider } from './registry';
import type { CopilotQuota, CopilotQuotaSnapshot, ProviderQuota, QuotaWindow } from './types';

const COPILOT_CACHE_KEY = 'copilot-quota';
const NOT_LOGGED_IN_ERROR = `Not logged in. Open \`${APP_NAME} menu\` and choose Provider login.`;
const PREFERRED_BUCKETS = ['premium_interactions', 'chat', 'completions'] as const;

interface CopilotConfigUser {
  host?: string;
  login?: string;
}

interface CopilotConfig {
  lastLoggedInUser?: CopilotConfigUser;
  loggedInUsers?: CopilotConfigUser[];
}

interface JsonRpcMessage {
  id?: number | string;
  result?: unknown;
  error?: {
    code?: number;
    message?: string;
    data?: unknown;
  };
}

interface CopilotQuotaRpcResult {
  quotaSnapshots?: Record<string, unknown>;
}

interface FrameState {
  buffer: Buffer;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, Math.round(value)));
}

function numberOr(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function stringOrNull(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function boolOrFalse(value: unknown): boolean {
  return typeof value === 'boolean' ? value : false;
}

function formatBucketLabel(bucket: string): string {
  if (bucket === 'premium_interactions') return 'Premium requests';
  if (bucket === 'chat') return 'Chat';
  if (bucket === 'completions') return 'Completions';
  return bucket
    .replace(/[_-]+/g, ' ')
    .trim()
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

export class CopilotProvider extends BaseProvider {
  readonly id = 'copilot';
  readonly name = 'Copilot';
  readonly cacheKey = COPILOT_CACHE_KEY;

  async isAvailable(): Promise<boolean> {
    if (!findCopilotBin()) return false;
    if (this.hasTokenEnv()) return true;
    return (await this.readAccountFromConfig()) !== undefined;
  }

  protected unavailableError(): string {
    if (!findCopilotBin()) return COPILOT_MISSING_ERROR;
    return NOT_LOGGED_IN_ERROR;
  }

  protected async fetchRaw(): Promise<unknown> {
    const bin = findCopilotBin();
    if (!bin) {
      throw new Error('Copilot CLI not found');
    }
    const base: CopilotQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };
    return this.fetchUsage(base, bin);
  }

  protected buildQuota(raw: unknown, _base: QuotaBase): ProviderQuota {
    return raw as ProviderQuota;
  }

  protected toUserFacingError(error: unknown): string {
    const message = error instanceof Error ? error.message : String(error);
    if (/not logged in|auth|unauthorized|forbidden|token|login|credential/i.test(message)) {
      return NOT_LOGGED_IN_ERROR;
    }
    if (/timed out/i.test(message)) {
      return 'GitHub Copilot CLI timed out while fetching usage';
    }
    return 'Failed to fetch Copilot usage';
  }

  private hasTokenEnv(): boolean {
    return Boolean(process.env.COPILOT_GITHUB_TOKEN || process.env.GH_TOKEN || process.env.GITHUB_TOKEN);
  }

  private async readAccountFromConfig(): Promise<string | undefined> {
    for (const path of [CONFIG.paths.copilot.config, CONFIG.paths.copilot.settings]) {
      try {
        const file = Bun.file(path);
        if (!(await file.exists())) continue;

        const text = await file.text();
        const config = JSON.parse(text.replace(/^\s*\/\/.*$/gm, '')) as CopilotConfig;
        const user = config.lastLoggedInUser ?? config.loggedInUsers?.find((entry) => entry.login);
        if (user?.login) return user.login;
      } catch {}
    }

    return undefined;
  }

  private async fetchUsage(base: CopilotQuota, bin: string): Promise<CopilotQuota> {
    const result = await this.fetchQuotaViaCli(bin);
    const quotaSnapshots = this.normalizeQuotaSnapshots(result.quotaSnapshots);

    if (Object.keys(quotaSnapshots).length === 0) {
      return { ...base, error: 'No Copilot quota snapshots found' };
    }

    const models = this.buildModels(quotaSnapshots);
    const primary = models['Premium requests'] ?? Object.values(models)[0];
    if (!primary) {
      return { ...base, error: 'No Copilot quota windows found' };
    }

    const account = await this.readAccountFromConfig();
    const meta: Record<string, string> = {
      primaryBucket: quotaSnapshots.premium_interactions ? 'premium_interactions' : Object.keys(quotaSnapshots)[0],
    };

    return {
      ...base,
      available: true,
      ...(account ? { account } : {}),
      primary,
      models,
      extra: {
        meta,
        quotaSnapshots,
      },
    };
  }

  private normalizeQuotaSnapshots(raw: Record<string, unknown> | undefined): Record<string, CopilotQuotaSnapshot> {
    if (!raw) return {};

    const snapshots: Record<string, CopilotQuotaSnapshot> = {};
    for (const [bucket, value] of Object.entries(raw)) {
      const snapshot = this.normalizeSnapshot(value);
      if (snapshot) snapshots[bucket] = snapshot;
    }
    return snapshots;
  }

  private normalizeSnapshot(raw: unknown): CopilotQuotaSnapshot | null {
    if (!isRecord(raw)) return null;

    const entitlementRequests = numberOr(raw.entitlementRequests, 0);
    const usedRequests = numberOr(raw.usedRequests, 0);
    const isUnlimitedEntitlement = boolOrFalse(raw.isUnlimitedEntitlement);
    const fallbackRemaining = isUnlimitedEntitlement
      ? 100
      : entitlementRequests > 0
        ? ((entitlementRequests - usedRequests) / entitlementRequests) * 100
        : 0;

    const snapshot: CopilotQuotaSnapshot = {
      isUnlimitedEntitlement,
      entitlementRequests,
      usedRequests,
      usageAllowedWithExhaustedQuota: boolOrFalse(raw.usageAllowedWithExhaustedQuota),
      overage: numberOr(raw.overage, 0),
      overageAllowedWithExhaustedQuota: boolOrFalse(raw.overageAllowedWithExhaustedQuota),
      remainingPercentage: numberOr(raw.remainingPercentage, fallbackRemaining),
      resetDate: stringOrNull(raw.resetDate),
    };

    if (typeof raw.hasQuota === 'boolean') {
      snapshot.hasQuota = raw.hasQuota;
    }
    if (typeof raw.tokenBasedBilling === 'boolean') {
      snapshot.tokenBasedBilling = raw.tokenBasedBilling;
    }

    return snapshot;
  }

  private buildModels(snapshots: Record<string, CopilotQuotaSnapshot>): Record<string, QuotaWindow> {
    const models: Record<string, QuotaWindow> = {};
    const orderedBuckets = [
      ...PREFERRED_BUCKETS.filter((bucket) => snapshots[bucket]),
      ...Object.keys(snapshots).filter(
        (bucket) => !PREFERRED_BUCKETS.includes(bucket as (typeof PREFERRED_BUCKETS)[number]),
      ),
    ];

    for (const bucket of orderedBuckets) {
      const snapshot = snapshots[bucket];
      models[formatBucketLabel(bucket)] = {
        remaining: snapshot.isUnlimitedEntitlement ? 100 : clampPercent(snapshot.remainingPercentage),
        resetsAt: snapshot.resetDate,
      };
    }

    return models;
  }

  private async fetchQuotaViaCli(
    bin: string,
    timeoutMs: number = CONFIG.api.timeoutMs,
  ): Promise<CopilotQuotaRpcResult> {
    const { spawn } = await import('node:child_process');

    return await new Promise<CopilotQuotaRpcResult>((resolve, reject) => {
      const proc = spawn(bin, ['--headless', '--stdio', '--no-auto-update', '--log-level', 'error'], {
        stdio: ['pipe', 'pipe', 'pipe'],
        env: { ...process.env, NO_COLOR: '1', TERM: 'dumb' },
      });

      const state: FrameState = { buffer: Buffer.alloc(0) };
      let finished = false;
      let stderr = '';

      const cleanup = () => {
        if (finished) return false;
        finished = true;
        clearTimeout(timer);
        try {
          proc.kill();
        } catch {}
        return true;
      };

      const finishResolve = (value: CopilotQuotaRpcResult) => {
        if (!cleanup()) return;
        resolve(value);
      };

      const finishReject = (error: Error) => {
        if (!cleanup()) return;
        reject(error);
      };

      const timer = setTimeout(() => {
        finishReject(new Error(`Copilot CLI timed out after ${timeoutMs}ms`));
      }, timeoutMs);

      const send = (id: number, method: string, params: Record<string, unknown> = {}) => {
        const body = JSON.stringify({ jsonrpc: '2.0', id, method, params });
        const frame = `Content-Length: ${Buffer.byteLength(body, 'utf8')}\r\n\r\n${body}`;
        proc.stdin.write(frame);
      };

      proc.on('error', (error) => {
        finishReject(error instanceof Error ? error : new Error(String(error)));
      });

      proc.on('exit', (code) => {
        if (finished) return;
        const suffix = stderr.trim() ? `: ${stderr.trim()}` : '';
        finishReject(new Error(`Copilot CLI exited with code ${code ?? 'unknown'}${suffix}`));
      });

      proc.stderr.on('data', (chunk: Buffer) => {
        stderr += chunk.toString('utf8');
        if (stderr.length > 2000) stderr = stderr.slice(-2000);
      });

      proc.stdout.on('data', (chunk: Buffer) => {
        try {
          for (const message of this.readFrames(state, chunk)) {
            if (message.id !== 2) continue;

            if (message.error) {
              finishReject(new Error(message.error.message ?? 'Copilot quota request failed'));
              return;
            }

            finishResolve((message.result ?? {}) as CopilotQuotaRpcResult);
            return;
          }
        } catch (error) {
          finishReject(error instanceof Error ? error : new Error(String(error)));
        }
      });

      send(1, 'ping');
      send(2, 'account.getQuota');
    });
  }

  private readFrames(state: FrameState, chunk: Buffer): JsonRpcMessage[] {
    state.buffer = Buffer.concat([state.buffer, chunk]);
    const messages: JsonRpcMessage[] = [];

    while (state.buffer.length > 0) {
      const headerEnd = this.findHeaderEnd(state.buffer);
      if (!headerEnd) break;

      const header = state.buffer.subarray(0, headerEnd.index).toString('utf8');
      const match = header.match(/content-length:\s*(\d+)/i);

      if (!match) {
        const next = state.buffer.indexOf('Content-Length:', 1, 'utf8');
        if (next === -1) {
          state.buffer = Buffer.alloc(0);
          break;
        }
        state.buffer = state.buffer.subarray(next);
        continue;
      }

      const contentLength = Number.parseInt(match[1], 10);
      const bodyStart = headerEnd.index + headerEnd.length;
      const bodyEnd = bodyStart + contentLength;

      if (state.buffer.length < bodyEnd) break;

      const body = state.buffer.subarray(bodyStart, bodyEnd).toString('utf8');
      messages.push(JSON.parse(body) as JsonRpcMessage);
      state.buffer = state.buffer.subarray(bodyEnd);
    }

    return messages;
  }

  private findHeaderEnd(buffer: Buffer): { index: number; length: number } | null {
    const crlf = buffer.indexOf('\r\n\r\n', 0, 'utf8');
    const lf = buffer.indexOf('\n\n', 0, 'utf8');

    if (crlf === -1 && lf === -1) return null;
    if (crlf !== -1 && (lf === -1 || crlf < lf)) return { index: crlf, length: 4 };
    return { index: lf, length: 2 };
  }
}

registerProvider(new CopilotProvider());

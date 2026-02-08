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
  secondary: {
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

  private unixToIso(timestamp: number): string {
    return new Date(timestamp * 1000).toISOString();
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
      // Find and parse latest session file
      const sessionFile = await this.findLatestSessionFile();
      if (!sessionFile) {
        return { ...base, error: 'No session data found' };
      }

      limits = await this.extractRateLimits(sessionFile);
      if (!limits) {
        return { ...base, error: 'No rate limit data in session' };
      }

      // Cache the result
      await cache.set('codex-quota', limits, CONFIG.cache.codexTtlMs);
    }

    // Build quota response
    const primary: QuotaWindow = {
      remaining: 100 - Math.round(limits.primary.used_percent),
      resetsAt: this.unixToIso(limits.primary.resets_at),
    };

    const secondary: QuotaWindow = {
      remaining: 100 - Math.round(limits.secondary.used_percent),
      resetsAt: this.unixToIso(limits.secondary.resets_at),
    };

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
      secondary,
      extraUsage: codexCredits,
    };
  }
}

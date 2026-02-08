import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

function findAmpBin(): string | null {
  if (typeof Bun.which === 'function') {
    const found = Bun.which('amp');
    if (found) return found;
  }

  const home = process.env.HOME ?? '';
  const paths = [
    `${home}/.cache/.bun/bin/amp`,
    `${home}/.bun/bin/amp`,
  ];

  const { existsSync } = require('node:fs');
  for (const p of paths) {
    if (existsSync(p)) return p;
  }

  return null;
}

export class AmpProvider implements Provider {
  readonly id = 'amp';
  readonly name = 'Amp';

  async isAvailable(): Promise<boolean> {
    return findAmpBin() !== null;
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    const bin = findAmpBin();
    if (!bin) {
      return { ...base, error: 'Amp CLI not installed' };
    }

    try {
      return await cache.getOrFetch<ProviderQuota>(
        'amp-quota',
        async () => await this.fetchUsage(base, bin),
        CONFIG.cache.ttlMs
      );
    } catch (error) {
      logger.error('Amp quota fetch error', { error });
      return { ...base, error: 'Failed to fetch usage' };
    }
  }

  private async fetchUsage(base: ProviderQuota, bin: string): Promise<ProviderQuota> {
    try {
      const proc = Bun.spawn([bin, 'usage'], {
        stdout: 'pipe',
        stderr: 'pipe',
        env: { ...process.env, NO_COLOR: '1', TERM: 'dumb' },
      });

      const stdout = await new Response(proc.stdout).text();
      const exitCode = await proc.exited;

      if (exitCode !== 0) {
        return { ...base, error: 'Not logged in' };
      }

      const accountMatch = stdout.match(/Signed in as (\S+)/);
      const account = accountMatch?.[1] || undefined;

      if (!account) {
        return { ...base, error: 'Not logged in' };
      }

      const freeMatch = stdout.match(/Amp Free:\s*\$([0-9.]+)\/\$([0-9.]+)\s*remaining/);
      const replenishMatch = stdout.match(/replenishes \+\$([0-9.]+)\/hour/);
      const replenishRate = replenishMatch ? `+$${replenishMatch[1]}/hr` : null;
      const bonusMatch = stdout.match(/\+(\d+)%\s*bonus\s*for\s*(\d+)\s*more\s*days/);
      const bonus = bonusMatch ? `+${bonusMatch[1]}% (${bonusMatch[2]}d)` : null;

      let primary: QuotaWindow | undefined;

      if (freeMatch) {
        const remaining = parseFloat(freeMatch[1]);
        const total = parseFloat(freeMatch[2]);
        const pct = total > 0 ? Math.round((remaining / total) * 100) : 0;
        
        // Calculate ETA to 100%
        let fullAt: string | null = null;
        if (replenishMatch && remaining < total) {
          const ratePerHour = parseFloat(replenishMatch[1]);
          const effectiveRate = bonusMatch ? ratePerHour * (1 + parseInt(bonusMatch[1]) / 100) : ratePerHour;
          const deficit = total - remaining;
          const hoursToFull = deficit / effectiveRate;
          fullAt = new Date(Date.now() + hoursToFull * 3600_000).toISOString();
        }
        
        primary = {
          remaining: pct,
          resetsAt: fullAt,
        };
      }

      const creditsMatch = stdout.match(/Individual credits:\s*\$([0-9.]+)\s*remaining/);
      let extraUsage: ProviderQuota['extraUsage'] | undefined;

      if (creditsMatch) {
        const balance = parseFloat(creditsMatch[1]);
        if (balance > 0) {
          extraUsage = {
            enabled: true,
            remaining: 100,
            limit: 0,
            used: 0,
          };
        }
      }

      const models: Record<string, QuotaWindow> = {};

      if (freeMatch) {
        const remaining = parseFloat(freeMatch[1]);
        const total = parseFloat(freeMatch[2]);
        const pct = total > 0 ? Math.round((remaining / total) * 100) : 0;
        
        // Calculate ETA to 100% based on replenish rate
        let fullAt: string | null = null;
        if (replenishMatch && remaining < total) {
          const ratePerHour = parseFloat(replenishMatch[1]);
          // With bonus, effective rate is doubled
          const effectiveRate = bonusMatch ? ratePerHour * (1 + parseInt(bonusMatch[1]) / 100) : ratePerHour;
          const deficit = total - remaining;
          const hoursToFull = deficit / effectiveRate;
          const msToFull = hoursToFull * 3600_000;
          fullAt = new Date(Date.now() + msToFull).toISOString();
        }
        
        let label = `Free $${remaining}/$${total}`;
        if (replenishRate) label += ` (${replenishRate})`;
        if (bonus) label += ` ${bonus}`;
        models[label] = { remaining: pct, resetsAt: fullAt };
      }

      if (creditsMatch) {
        const balance = parseFloat(creditsMatch[1]);
        models[`Credits $${balance}`] = { remaining: balance > 0 ? 100 : 0, resetsAt: null };
      }

      return {
        ...base,
        available: true,
        account,
        primary,
        extraUsage,
        models,
      };
    } catch (error) {
      logger.error('Amp usage parse error', { error });
      return { ...base, error: 'Failed to parse usage' };
    }
  }
}

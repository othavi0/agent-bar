import { AMP_MISSING_ERROR, findAmpBin } from '../amp-cli';
import { APP_NAME } from '../app-identity';
import { CONFIG } from '../config';
import { logger } from '../logger';
import type { QuotaBase } from './base';
import { BaseProvider } from './base';
import { registerProvider } from './registry';
import type { AmpQuota, AmpQuotaExtra, ProviderQuota, QuotaWindow } from './types';

const NOT_LOGGED_IN_ERROR = `Not logged in. Open \`${APP_NAME} menu\` and choose Provider login.`;
/** Sentinel thrown by fetchRaw so auth failures bypass the cache and map cleanly. */
const NOT_LOGGED_IN_SENTINEL = 'amp:not-logged-in';

export class AmpProvider extends BaseProvider {
  readonly id = 'amp';
  readonly name = 'Amp';
  readonly cacheKey = 'amp-quota';

  async isAvailable(): Promise<boolean> {
    return findAmpBin() !== null;
  }

  protected unavailableError(): string {
    return AMP_MISSING_ERROR;
  }

  protected toUserFacingError(error: unknown): string {
    if (error instanceof Error && error.message === NOT_LOGGED_IN_SENTINEL) {
      return NOT_LOGGED_IN_ERROR;
    }
    return 'Failed to fetch Amp usage';
  }

  /**
   * Run `amp usage` and return its raw stdout. Throws (so the cache is not
   * poisoned) when the CLI errors or the user is not logged in. The subprocess
   * is always killed — a hung `amp usage` would otherwise leak a zombie on
   * every Waybar poll.
   */
  protected async fetchRaw(): Promise<unknown> {
    const bin = findAmpBin();
    if (!bin) {
      throw new Error('Amp CLI not found');
    }

    const proc = Bun.spawn([bin, 'usage'], {
      stdout: 'pipe',
      stderr: 'pipe',
      env: { ...process.env, NO_COLOR: '1', TERM: 'dumb' },
    });

    const killTimer = setTimeout(() => {
      try {
        proc.kill();
      } catch {}
    }, CONFIG.api.timeoutMs);

    try {
      // Drain stderr so a large error dump can't deadlock the stdout pipe.
      void new Response(proc.stderr).text().catch(() => {});
      const stdout = await new Response(proc.stdout).text();
      const exitCode = await proc.exited;

      if (exitCode !== 0 || !/Signed in as (\S+)/.test(stdout)) {
        throw new Error(NOT_LOGGED_IN_SENTINEL);
      }

      return stdout;
    } finally {
      clearTimeout(killTimer);
      try {
        proc.kill();
      } catch {}
    }
  }

  protected buildQuota(raw: unknown, base: QuotaBase): ProviderQuota {
    const stdout = typeof raw === 'string' ? raw : '';
    try {
      return this.parseUsage(stdout, base);
    } catch (error) {
      logger.error('Amp usage parse error', { error });
      return { ...base, error: 'Failed to parse usage' } as AmpQuota;
    }
  }

  private parseUsage(stdout: string, base: QuotaBase): AmpQuota {
    const accountMatch = stdout.match(/Signed in as (\S+)/);
    const account = accountMatch?.[1] || undefined;

    const freeMatch = stdout.match(/Amp Free:\s*\$([0-9.]+)\/\$([0-9.]+)\s*remaining/);
    const replenishMatch = stdout.match(/replenishes \+\$([0-9.]+)\/hour/);
    const replenishRate = replenishMatch ? `+$${replenishMatch[1]}/hr` : null;
    const bonusMatch = stdout.match(/\+(\d+)%\s*bonus\s*for\s*(\d+)\s*more\s*days/);
    const bonus = bonusMatch ? `+${bonusMatch[1]}% (${bonusMatch[2]}d)` : null;

    const parseAmpFreeTier = (
      match: RegExpMatchArray,
      replenish: RegExpMatchArray | null,
      bonusM: RegExpMatchArray | null,
    ): { pct: number; fullAt: string | null } => {
      const remaining = parseFloat(match[1]);
      const total = parseFloat(match[2]);
      const pct = total > 0 ? Math.round((remaining / total) * 100) : 0;
      let fullAt: string | null = null;
      if (replenish && remaining < total) {
        const ratePerHour = parseFloat(replenish[1]);
        const effectiveRate = bonusM ? ratePerHour * (1 + parseInt(bonusM[1], 10) / 100) : ratePerHour;
        const hoursToFull = (total - remaining) / effectiveRate;
        if (effectiveRate > 0 && Number.isFinite(hoursToFull)) {
          fullAt = new Date(Date.now() + hoursToFull * 3_600_000).toISOString();
        }
      }
      return { pct, fullAt };
    };

    const creditsMatch = stdout.match(/Individual credits:\s*\$([0-9.]+)\s*remaining/);

    const models: Record<string, QuotaWindow> = {};
    const meta: Record<string, string> = {};
    const extra: AmpQuotaExtra = {};
    let primary: QuotaWindow | undefined;

    if (freeMatch) {
      const remaining = parseFloat(freeMatch[1]);
      const total = parseFloat(freeMatch[2]);
      const { pct, fullAt } = parseAmpFreeTier(freeMatch, replenishMatch, bonusMatch);

      primary = { remaining: pct, resetsAt: fullAt };
      models['Free Tier'] = { remaining: pct, resetsAt: fullAt };
      meta.freeRemaining = `$${remaining}`;
      meta.freeTotal = `$${total}`;
      if (replenishRate) meta.replenishRate = replenishRate;
      if (bonus) meta.bonus = bonus;
    }

    if (creditsMatch) {
      const balance = parseFloat(creditsMatch[1]);
      models.Credits = { remaining: balance > 0 ? 100 : 0, resetsAt: null };
      meta.creditsBalance = `$${balance}`;
    }

    if (Object.keys(meta).length > 0) extra.meta = meta;

    return {
      ...base,
      provider: this.id,
      available: true,
      account,
      primary,
      models,
      ...(Object.keys(extra).length > 0 ? { extra } : {}),
    };
  }
}

registerProvider(new AmpProvider());

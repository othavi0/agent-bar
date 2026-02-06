import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

interface AntigravityJsonModel {
  label: string;
  modelId: string;
  remainingPercentage: number;  // 0.0 - 1.0
  isExhausted: boolean;
  resetTime?: string;
  timeUntilResetMs?: number;
}

interface AntigravityJsonOutput {
  email: string;
  method: string;
  timestamp: string;
  models: AntigravityJsonModel[];
}

export class AntigravityProvider implements Provider {
  readonly id = 'antigravity';
  readonly name = 'Antigravity';

  async isAvailable(): Promise<boolean> {
    // Check if antigravity-usage is installed
    try {
      const proc = Bun.spawn(['which', 'antigravity-usage'], { stdout: 'ignore', stderr: 'ignore' });
      if (await proc.exited !== 0) return false;

      // Check if logged in (has accounts)
      const result = Bun.spawnSync(['antigravity-usage', 'accounts', 'list'], {
        stdout: 'pipe',
        stderr: 'ignore',
      });
      
      const output = result.stdout.toString();
      // If has accounts, output will contain email addresses
      return output.includes('@') || output.includes('gmail.com');
    } catch {
      return false;
    }
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    // Check if antigravity-usage is installed
    try {
      const whichProc = Bun.spawn(['which', 'antigravity-usage'], { stdout: 'ignore', stderr: 'ignore' });
      if (await whichProc.exited !== 0) {
        return { ...base, error: 'Not logged in - use qbar menu → Provider login' };
      }
    } catch {
      return { ...base, error: 'Not logged in - use qbar menu → Provider login' };
    }

    // Use cache
    const cacheKey = 'antigravity-quota';
    
    try {
      return await cache.getOrFetch<ProviderQuota>(
        cacheKey,
        async () => await this.fetchQuotaFromCLI(base),
        CONFIG.cache.ttlMs
      );
    } catch (error) {
      logger.error('Antigravity quota fetch error', { error });
      return { ...base, error: 'Failed to fetch quota' };
    }
  }

  private async fetchQuotaFromCLI(base: ProviderQuota): Promise<ProviderQuota> {
    try {
      const result = Bun.spawnSync(['antigravity-usage', '--json'], {
        stdout: 'pipe',
        stderr: 'pipe',
        timeout: CONFIG.api.timeoutMs,
      });

      if (result.exitCode !== 0) {
        const stderr = result.stderr.toString();
        logger.debug('antigravity-usage failed', { exitCode: result.exitCode, stderr });
        
        if (stderr.includes('No accounts') || stderr.includes('login')) {
          return { ...base, error: 'Not logged in - use qbar menu → Provider login' };
        }
        return { ...base, error: 'Failed to fetch quota' };
      }

      const output = result.stdout.toString().trim();
      
      // Parse JSON output
      let data: AntigravityJsonOutput;
      try {
        data = JSON.parse(output);
      } catch {
        logger.debug('Failed to parse antigravity-usage JSON', { output });
        return { ...base, error: 'Invalid response from antigravity-usage' };
      }

      // Extract models
      const models: Record<string, QuotaWindow> = {};
      
      for (const model of data.models || []) {
        // Use the label from antigravity-usage directly (already pretty)
        const label = model.label;
        
        // Convert 0.0-1.0 to 0-100 percentage
        const remaining = Math.round(model.remainingPercentage * 100);

        models[label] = {
          remaining,
          resetsAt: model.resetTime || null,
        };
      }

      // Find primary model (prefer Claude Opus Thinking)
      const primaryKey = Object.keys(models).find(k => k.includes('Claude Opus Thinking'))
        || Object.keys(models).find(k => k.includes('Claude'))
        || Object.keys(models).find(k => k.includes('Gemini 3 Pro'))
        || Object.keys(models)[0];

      const primary = primaryKey ? models[primaryKey] : undefined;

      if (Object.keys(models).length === 0) {
        return { ...base, account: data.email, error: 'No model quotas available' };
      }

      return {
        ...base,
        available: true,
        account: data.email,
        primary,
        models,
      };
    } catch (error) {
      logger.error('antigravity-usage execution error', { error });
      return { ...base, error: 'Failed to fetch quota' };
    }
  }
}

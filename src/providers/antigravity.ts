import { CONFIG } from '../config';
import { logger } from '../logger';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

interface AntigravityQuotaInfo {
  remainingFraction?: number;
  resetTime?: string;
}

interface AntigravityModelConfig {
  label: string;
  quotaInfo?: AntigravityQuotaInfo;
}

interface AntigravityUserStatus {
  name?: string;
  email?: string;
  cascadeModelConfigData?: {
    clientModelConfigs?: AntigravityModelConfig[];
  };
}

interface AntigravityResponse {
  userStatus?: AntigravityUserStatus;
}

// Model patterns to look for
const MODEL_PATTERNS = {
  claude: /claude.*opus.*thinking/i,
  geminiPro: /gemini.*3.*pro.*high/i,
  geminiFlash: /gemini.*3.*flash/i,
};

export class AntigravityProvider implements Provider {
  readonly id = 'antigravity';
  readonly name = 'Antigravity';

  async isAvailable(): Promise<boolean> {
    // Check if language server is running
    const process = await this.findLanguageServer();
    return process !== null;
  }

  private async findLanguageServer(): Promise<{ pid: string; csrfToken: string } | null> {
    try {
      const result = Bun.spawnSync(['pgrep', '-af', 'language_server_linux']);
      const output = result.stdout.toString();
      
      if (!output) return null;

      // Extract CSRF token from process args
      const csrfMatch = output.match(/--csrf_token\s+([a-f0-9-]+)/);
      if (!csrfMatch) return null;

      const pidMatch = output.match(/^(\d+)/);
      if (!pidMatch) return null;

      return {
        pid: pidMatch[1],
        csrfToken: csrfMatch[1],
      };
    } catch {
      return null;
    }
  }

  private async findListeningPorts(): Promise<number[]> {
    try {
      const result = Bun.spawnSync(['lsof', '-nP', '-iTCP', '-sTCP:LISTEN']);
      const output = result.stdout.toString();
      
      const ports: number[] = [];
      for (const line of output.split('\n')) {
        if (line.includes('language_')) {
          const portMatch = line.match(/:(\d+)\s/);
          if (portMatch) {
            ports.push(parseInt(portMatch[1], 10));
          }
        }
      }
      
      return ports.slice(0, 3); // Max 3 ports
    } catch {
      return [];
    }
  }

  private parseModelQuota(models: AntigravityModelConfig[], pattern: RegExp): QuotaWindow | undefined {
    const model = models.find(m => pattern.test(m.label));
    if (!model?.quotaInfo) return undefined;

    const fraction = model.quotaInfo.remainingFraction ?? 1;
    return {
      remaining: Math.floor(fraction * 100),
      resetsAt: model.quotaInfo.resetTime || null,
    };
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    // Find language server process
    const server = await this.findLanguageServer();
    if (!server) {
      return { ...base, error: 'Language server not running' };
    }

    // Find listening ports
    const ports = await this.findListeningPorts();
    if (ports.length === 0) {
      return { ...base, error: 'No ports found' };
    }

    // Try each port until we get a response
    for (const port of ports) {
      try {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), CONFIG.api.timeoutMs);

        const response = await fetch(
          `https://127.0.0.1:${port}/exa.language_server_pb.LanguageServerService/GetUserStatus`,
          {
            method: 'POST',
            headers: {
              'X-Codeium-Csrf-Token': server.csrfToken,
              'Connect-Protocol-Version': '1',
              'Content-Type': 'application/json',
            },
            body: JSON.stringify({ metadata: { ideName: 'antigravity' } }),
            signal: controller.signal,
            tls: { rejectUnauthorized: false },
          }
        );

        clearTimeout(timeout);

        if (!response.ok) continue;

        const data: AntigravityResponse = await response.json();
        if (!data.userStatus?.email) continue;

        const account = data.userStatus.name?.split(' ')[0] || data.userStatus.email;
        const models = data.userStatus.cascadeModelConfigData?.clientModelConfigs || [];

        // Extract model quotas
        const modelQuotas: Record<string, QuotaWindow> = {};
        
        const claude = this.parseModelQuota(models, MODEL_PATTERNS.claude);
        if (claude) modelQuotas['Claude Opus'] = claude;

        const geminiPro = this.parseModelQuota(models, MODEL_PATTERNS.geminiPro);
        if (geminiPro) modelQuotas['Gemini Pro'] = geminiPro;

        const geminiFlash = this.parseModelQuota(models, MODEL_PATTERNS.geminiFlash);
        if (geminiFlash) modelQuotas['Gemini Flash'] = geminiFlash;

        // Use Claude as primary if available
        const primary = claude || geminiPro || geminiFlash;

        return {
          ...base,
          available: true,
          account,
          primary,
          models: modelQuotas,
        };
      } catch (error) {
        if (error instanceof Error && error.name === 'AbortError') {
          logger.debug('Antigravity port timeout', { port });
        }
        continue;
      }
    }

    // Fallback: try antigravity-usage CLI if available
    return await this.fallbackToCliTool(base);
  }

  private async fallbackToCliTool(base: ProviderQuota): Promise<ProviderQuota> {
    try {
      const result = Bun.spawnSync(['antigravity-usage', 'quota', '--json'], {
        timeout: CONFIG.api.timeoutMs,
      });

      if (result.exitCode !== 0) {
        return { ...base, error: 'Failed to get quota from LSP' };
      }

      const output = result.stdout.toString();
      const data = JSON.parse(output);
      
      // Handle array response (multiple accounts)
      const account = Array.isArray(data) ? data[0] : data;
      if (!account) {
        return { ...base, error: 'No account data' };
      }

      const models: Record<string, QuotaWindow> = {};
      const accountModels = account.models || account.snapshot?.models || [];

      for (const model of accountModels) {
        if (!model.label || model.remainingPercentage === undefined) continue;
        
        let remaining = model.remainingPercentage;
        // Normalize 0-1 to 0-100 if needed
        if (remaining <= 1) remaining = Math.floor(remaining * 100);

        models[model.label] = {
          remaining,
          resetsAt: model.resetTime || null,
        };
      }

      // Find primary (prefer Claude)
      const primaryKey = Object.keys(models).find(k => /claude/i.test(k)) 
        || Object.keys(models)[0];
      const primary = primaryKey ? models[primaryKey] : undefined;

      return {
        ...base,
        available: true,
        account: account.email || account.accountEmail || 'unknown',
        primary,
        models,
      };
    } catch (error) {
      logger.debug('Antigravity CLI fallback failed', { error });
      return { ...base, error: 'Failed to get quota' };
    }
  }
}

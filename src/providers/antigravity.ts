import { homedir } from 'os';
import { join } from 'path';
import { CONFIG } from '../config';
import { logger } from '../logger';
import { cache } from '../cache';
import type { Provider, ProviderQuota, QuotaWindow } from './types';

const BASE_URL = 'https://cloudcode-pa.googleapis.com';
const LOAD_CODE_ASSIST = '/v1internal:loadCodeAssist';
const FETCH_MODELS = '/v1internal:fetchAvailableModels';

interface AuthProfile {
  type: string;
  provider: string;
  access: string;
  refresh: string;
  expires: number;
  email: string;
  projectId?: string;
}

interface AuthProfiles {
  profiles: Record<string, AuthProfile>;
}

interface ModelQuotaInfo {
  remainingFraction?: number;
  resetTime?: string;
}

interface ModelInfo {
  quotaInfo?: ModelQuotaInfo;
}

interface LoadCodeAssistResponse {
  cloudaicompanionProject?: string | { id?: string };
  availablePromptCredits?: number;
  planInfo?: { monthlyPromptCredits?: number };
  currentTier?: { name?: string };
  planType?: string;
}

interface FetchModelsResponse {
  models?: Record<string, ModelInfo>;
}

export class AntigravityProvider implements Provider {
  readonly id = 'antigravity';
  readonly name = 'Antigravity';

  private authProfilesPath = join(homedir(), '.openclaw', 'agents', 'main', 'agent', 'auth-profiles.json');

  async isAvailable(): Promise<boolean> {
    const profile = await this.getActiveProfile();
    return profile !== null;
  }

  private async getActiveProfile(): Promise<AuthProfile | null> {
    try {
      const file = Bun.file(this.authProfilesPath);
      if (!await file.exists()) {
        logger.debug('Antigravity: auth-profiles.json not found');
        return null;
      }

      const data: AuthProfiles = await file.json();
      
      // Find first google-antigravity profile with valid token
      for (const [key, profile] of Object.entries(data.profiles)) {
        if (key.startsWith('google-antigravity:') && profile.access) {
          // Check if token is expired (with 5min buffer)
          const now = Date.now();
          if (profile.expires && profile.expires > now + 300_000) {
            return profile;
          }
          // Token expired but has refresh - OpenClaw will refresh it
          if (profile.refresh) {
            logger.debug('Antigravity: token expired, needs refresh');
            return profile;
          }
        }
      }

      return null;
    } catch (error) {
      logger.debug('Antigravity: failed to read auth-profiles', { error });
      return null;
    }
  }

  async getQuota(): Promise<ProviderQuota> {
    const base: ProviderQuota = {
      provider: this.id,
      displayName: this.name,
      available: false,
    };

    const profile = await this.getActiveProfile();
    if (!profile) {
      return { ...base, error: 'No Antigravity account in OpenClaw' };
    }

    // Use cache to avoid hitting API too often
    const cacheKey = `antigravity-${profile.email}`;
    
    try {
      const result = await cache.getOrFetch<ProviderQuota>(
        cacheKey,
        async () => await this.fetchQuotaFromAPI(profile, base),
        CONFIG.cache.ttlMs
      );
      return result;
    } catch (error) {
      logger.error('Antigravity quota fetch error', { error });
      return { ...base, account: profile.email, error: 'Failed to fetch quota' };
    }
  }

  private async fetchQuotaFromAPI(profile: AuthProfile, base: ProviderQuota): Promise<ProviderQuota> {
    const headers = {
      'Authorization': `Bearer ${profile.access}`,
      'Content-Type': 'application/json',
      'User-Agent': 'antigravity',
      'X-Goog-Api-Client': 'google-cloud-sdk vscode_cloudshelleditor/0.1',
    };

    let projectId = profile.projectId;
    let plan: string | undefined;
    const models: Record<string, QuotaWindow> = {};

    // 1. Fetch loadCodeAssist for plan info
    try {
      const res = await fetch(`${BASE_URL}${LOAD_CODE_ASSIST}`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ metadata: { ideType: 'ANTIGRAVITY', platform: 'PLATFORM_UNSPECIFIED', pluginType: 'GEMINI' } }),
        signal: AbortSignal.timeout(CONFIG.api.timeoutMs),
      });

      if (res.status === 401) {
        return { ...base, account: profile.email, error: 'Token expired - run: openclaw models auth login google-antigravity' };
      }

      if (res.ok) {
        const data: LoadCodeAssistResponse = await res.json();
        
        // Extract project ID
        if (data.cloudaicompanionProject) {
          if (typeof data.cloudaicompanionProject === 'string') {
            projectId = data.cloudaicompanionProject;
          } else if (data.cloudaicompanionProject.id) {
            projectId = data.cloudaicompanionProject.id;
          }
        }

        // Extract plan
        plan = data.currentTier?.name || data.planType;
      }
    } catch (error) {
      logger.debug('Antigravity: loadCodeAssist failed', { error });
    }

    // 2. Fetch model quotas
    try {
      const body = projectId ? { project: projectId } : {};
      const res = await fetch(`${BASE_URL}${FETCH_MODELS}`, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(CONFIG.api.timeoutMs),
      });

      if (res.ok) {
        const data: FetchModelsResponse = await res.json();
        
        if (data.models) {
          for (const [modelId, info] of Object.entries(data.models)) {
            // Skip internal models
            const lower = modelId.toLowerCase();
            if (lower.includes('chat_') || lower.includes('tab_')) continue;

            const quota = info.quotaInfo;
            if (quota?.remainingFraction !== undefined) {
              const remaining = Math.round(quota.remainingFraction * 100);
              
              // Pretty label
              let label = modelId;
              if (lower.includes('claude') && lower.includes('opus')) {
                label = lower.includes('thinking') ? 'Claude Opus Thinking' : 'Claude Opus';
              } else if (lower.includes('gemini-3-pro')) {
                label = 'Gemini 3 Pro';
              } else if (lower.includes('gemini-3-flash')) {
                label = 'Gemini 3 Flash';
              } else if (lower.includes('gemini-2.5-flash')) {
                label = 'Gemini 2.5 Flash';
              }

              models[label] = {
                remaining,
                resetsAt: quota.resetTime || null,
              };
            }
          }
        }
      }
    } catch (error) {
      logger.debug('Antigravity: fetchAvailableModels failed', { error });
    }

    // Find primary model (prefer Claude Opus Thinking)
    const primaryKey = Object.keys(models).find(k => k.includes('Claude Opus Thinking'))
      || Object.keys(models).find(k => k.includes('Claude'))
      || Object.keys(models).find(k => k.includes('Gemini 3 Pro'))
      || Object.keys(models)[0];

    const primary = primaryKey ? models[primaryKey] : undefined;

    if (Object.keys(models).length === 0) {
      return { ...base, account: profile.email, plan, error: 'No model quotas available' };
    }

    return {
      ...base,
      available: true,
      account: profile.email,
      plan,
      primary,
      models,
    };
  }
}

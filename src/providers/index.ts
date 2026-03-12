export * from './types';
export { ClaudeProvider } from './claude';
export { CodexProvider } from './codex';
export { AmpProvider } from './amp';

import { ClaudeProvider } from './claude';
import { CodexProvider } from './codex';
import { AmpProvider } from './amp';
import type { Provider, ProviderQuota, AllQuotas } from './types';

const PROVIDER_TIMEOUT_MS = 10_000;

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms)
    ),
  ]);
}

/**
 * All registered providers
 */
export const providers: Provider[] = [
  new ClaudeProvider(),
  new CodexProvider(),
  new AmpProvider(),
];

/**
 * Get provider by ID
 */
export function getProvider(id: string): Provider | undefined {
  return providers.find(p => p.id === id);
}

/**
 * Fetch quotas from all available providers
 */
export async function getAllQuotas(): Promise<AllQuotas> {
  const results = await Promise.all(
    providers.map(async (provider): Promise<ProviderQuota> => {
      try {
        return await withTimeout(provider.getQuota(), PROVIDER_TIMEOUT_MS, provider.name);
      } catch (error) {
        return {
          provider: provider.id,
          displayName: provider.name,
          available: false,
          error: error instanceof Error ? error.message : 'Unknown error',
        };
      }
    })
  );

  return {
    providers: results,
    fetchedAt: new Date().toISOString(),
  };
}

/**
 * Fetch quota from a specific provider
 */
export async function getQuotaFor(providerId: string): Promise<ProviderQuota | null> {
  const provider = getProvider(providerId);
  if (!provider) return null;

  try {
    return await withTimeout(provider.getQuota(), PROVIDER_TIMEOUT_MS, provider.name);
  } catch (error) {
    return {
      provider: providerId,
      displayName: provider.name,
      available: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

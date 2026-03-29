import type { AllQuotas, ProviderQuota, QuotaWindow } from '../../src/providers/types';

/**
 * Builds a minimal Bun.file()-like mock object.
 *
 * - `exists`: controls `file.exists()` return value
 * - `json`: controls `file.json()` return value (defaults to `{}`)
 * - `text`: controls `file.text()` return value (defaults to `''`)
 * - `throwOnJson`: makes `file.json()` reject with SyntaxError
 * - `stat`: controls `file.stat()` return value
 */
export function fakeFile(opts: {
  exists: boolean;
  json?: unknown;
  text?: string;
  throwOnJson?: boolean;
  stat?: { mtimeMs: number };
}) {
  return {
    exists: () => Promise.resolve(opts.exists),
    json: () => {
      if (opts.throwOnJson) return Promise.reject(new SyntaxError('Unexpected token'));
      return Promise.resolve(opts.json ?? {});
    },
    text: () => Promise.resolve(opts.text ?? ''),
    stat: () => Promise.resolve(opts.stat ?? { mtimeMs: Date.now() }),
  };
}

/**
 * Unix timestamp N hours from now. Useful for resetsAt fields.
 */
export function futureUnix(hoursFromNow = 1): number {
  return Math.floor(Date.now() / 1000) + hoursFromNow * 3600;
}

/**
 * ISO timestamp N hours from now.
 */
export function futureIso(hoursFromNow = 1): string {
  return new Date(Date.now() + hoursFromNow * 3600000).toISOString();
}

/**
 * Creates a QuotaWindow with sensible defaults.
 */
export function mockQuotaWindow(overrides: Partial<QuotaWindow> = {}): QuotaWindow {
  return {
    remaining: 75,
    resetsAt: futureIso(2),
    ...overrides,
  };
}

/**
 * Creates a ProviderQuota with sensible defaults.
 * Defaults to an available Claude provider at 75% remaining.
 */
export function mockProviderQuota(overrides: Partial<ProviderQuota> = {}): ProviderQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: true,
    primary: mockQuotaWindow(),
    ...overrides,
  };
}

/**
 * Creates an AllQuotas object wrapping the given providers.
 * Defaults to a single healthy Claude provider.
 */
export function mockAllQuotas(providers?: ProviderQuota[], fetchedAt?: string): AllQuotas {
  return {
    providers: providers ?? [mockProviderQuota()],
    fetchedAt: fetchedAt ?? new Date().toISOString(),
  };
}

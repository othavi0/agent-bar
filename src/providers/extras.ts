import type { AmpQuotaExtra, ClaudeQuotaExtra, CodexQuotaExtra, CopilotQuotaExtra, ProviderQuota } from './types';

// These getters concentrate the `extra` casts that were previously scattered across
// the formatters. The cast is unavoidable: `ProviderQuota` includes `GenericQuota`,
// whose `extra` is `Record<string, unknown>`, so discriminated-union narrowing alone
// is not enough to type the payload.

/** Returns the Claude-specific `extra` payload, or undefined for other providers. */
export function getClaudeExtra(q: ProviderQuota): ClaudeQuotaExtra | undefined {
  return q.provider === 'claude' ? (q.extra as ClaudeQuotaExtra | undefined) : undefined;
}

/** Returns the Codex-specific `extra` payload, or undefined for other providers. */
export function getCodexExtra(q: ProviderQuota): CodexQuotaExtra | undefined {
  return q.provider === 'codex' ? (q.extra as CodexQuotaExtra | undefined) : undefined;
}

/** Returns the Amp-specific `extra` payload, or undefined for other providers. */
export function getAmpExtra(q: ProviderQuota): AmpQuotaExtra | undefined {
  return q.provider === 'amp' ? (q.extra as AmpQuotaExtra | undefined) : undefined;
}

/** Returns the Copilot-specific `extra` payload, or undefined for other providers. */
export function getCopilotExtra(q: ProviderQuota): CopilotQuotaExtra | undefined {
  return q.provider === 'copilot' ? (q.extra as CopilotQuotaExtra | undefined) : undefined;
}

import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';

/** Bump on incompatible schema change (remove/rename/retype a stable field). Adding optional fields does not require a bump. */
export const SCHEMA_VERSION = 1;

export interface JsonWindow {
  remaining: number;
  used?: number | null;
  resetsAt: string | null;
  windowMinutes?: number | null;
}

export interface JsonProvider {
  provider: string;
  displayName: string;
  available: boolean;
  account?: string;
  plan?: string;
  planType?: string;
  primary?: JsonWindow;
  secondary?: JsonWindow;
  models?: Record<string, JsonWindow>;
  /** Provider-specific, pre-render data (weeklyModels, modelsDetailed, extraUsage, meta, quotaSnapshots). NOT covered by schemaVersion stability. */
  extra?: Record<string, unknown>;
  error?: string;
}

export interface JsonOutput {
  schemaVersion: number;
  fetchedAt: string;
  providers: JsonProvider[];
}

export function toJsonWindow(w: QuotaWindow): JsonWindow {
  const out: JsonWindow = { remaining: w.remaining, resetsAt: w.resetsAt };
  if (w.used !== undefined) out.used = w.used;
  if (w.windowMinutes !== undefined) out.windowMinutes = w.windowMinutes;
  return out;
}

export function toProviderOutput(p: ProviderQuota): JsonProvider {
  const out: JsonProvider = {
    provider: p.provider,
    displayName: p.displayName,
    available: p.available,
  };
  if (p.account !== undefined) out.account = p.account;
  if (p.plan !== undefined) out.plan = p.plan;
  if (p.planType !== undefined) out.planType = p.planType;
  if (p.primary !== undefined) out.primary = toJsonWindow(p.primary);
  if (p.secondary !== undefined) out.secondary = toJsonWindow(p.secondary);
  if (p.models !== undefined) {
    const models: Record<string, JsonWindow> = {};
    for (const [name, w] of Object.entries(p.models)) {
      models[name] = toJsonWindow(w);
    }
    out.models = models;
  }
  // Omit an empty extra object so the contract stays clean (only emit extra when it carries data).
  if (p.extra && Object.keys(p.extra).length > 0) {
    out.extra = p.extra as Record<string, unknown>;
  }
  if (p.error !== undefined) out.error = p.error;
  return out;
}

export function toJsonOutput(quotas: AllQuotas): JsonOutput {
  return {
    schemaVersion: SCHEMA_VERSION,
    fetchedAt: quotas.fetchedAt,
    providers: quotas.providers.map(toProviderOutput),
  };
}

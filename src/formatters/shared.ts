import type { ProviderQuota } from '../providers/types';
import type { DisplayMode } from '../settings';

export type { DisplayMode };

export function toDisplay(remaining: number | null, mode: DisplayMode): number | null {
  if (remaining === null) return null;
  return mode === 'used' ? 100 - remaining : remaining;
}

/**
 * Display value for a quota window, honoring a provider-supplied `used` percent
 * (Copilot) when present. Falls back to `100 - remaining` in `used` mode.
 */
export function toWindowDisplay(
  window: { remaining: number; used?: number | null } | undefined,
  mode: DisplayMode,
): number | null {
  if (!window) return null;
  if (mode === 'used' && window.used != null) return window.used;
  return toDisplay(window.remaining, mode);
}

export function toHealth(displayValue: number | null, mode: DisplayMode): number | null {
  if (displayValue === null) return null;
  return mode === 'used' ? 100 - displayValue : displayValue;
}

export function etaLabel(mode: DisplayMode): string {
  return mode === 'used' ? 'Resets in' : 'Full in';
}

export type WindowKind = 'fiveHour' | 'sevenDay' | 'other';

export function formatPercent(val: number | null): string {
  return val === null ? '?%' : `${Math.round(val)}%`;
}

export function formatEta(iso: string | null, remaining: number | null): string {
  if (remaining === 100) return 'Full';
  if (!iso) return '?';
  const diff = new Date(iso).getTime() - Date.now();
  if (diff < 0) return '0h 00m';
  const d = Math.floor(diff / 86400000);
  const h = Math.floor((diff % 86400000) / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  return d > 0 ? `${d}d ${h.toString().padStart(2, '0')}h` : `${h}h ${m.toString().padStart(2, '0')}m`;
}

export function formatResetTime(iso: string | null, remaining: number | null): string {
  if (remaining === 100) return '';
  if (!iso) return '(??:??)';
  const d = new Date(iso);
  return `(${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')})`;
}

export function classifyWindow(minutes: number | null | undefined): WindowKind {
  if (!minutes || minutes <= 0) return 'other';
  if (Math.abs(minutes - 300) <= 90) return 'fiveHour';
  if (Math.abs(minutes - 10080) <= 1440) return 'sevenDay';
  return 'other';
}

const PLAN_MAP: Record<string, string> = {
  free: 'Free',
  go: 'Go',
  plus: 'Plus',
  pro: 'Pro',
  business: 'Business',
  team: 'Business',
  enterprise: 'Enterprise',
  edu: 'Edu',
  education: 'Edu',
  apikey: 'API Key',
  api_key: 'API Key',
};

export function normalizePlan(raw: string | null | undefined): string | undefined {
  if (!raw) return undefined;
  const key = raw.trim().toLowerCase();
  if (!key) return undefined;
  return PLAN_MAP[key] ?? raw.replace(/[_-]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

export function normalizePlanLabel(p: ProviderQuota): string {
  return normalizePlan(p.plan ?? p.planType) ?? 'Unknown';
}

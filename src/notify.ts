import { spawn } from 'node:child_process';
import { rename } from 'node:fs/promises';
import { join } from 'node:path';
import { CONFIG } from './config';
import { logger } from './logger';
import { getClaudeExtra } from './providers/extras';
import type { AllQuotas, ProviderQuota, QuotaWindow } from './providers/types';

/** Used-percent thresholds for a quota window. */
const LOW_USED = 90; // >= 90% used (<= 10% remaining)
const CRITICAL_USED = 95; // >= 95% used (<= 5% remaining)

export type NotifyLevel = 'ok' | 'low' | 'critical';

const RANK: Record<NotifyLevel, number> = { ok: 0, low: 1, critical: 2 };

/** Persisted per-provider state: highest level already notified per window label. */
export interface ProviderNotifyState {
  windows: Record<string, NotifyLevel>;
}

export interface NotifyFire {
  provider: string;
  displayName: string;
  label: string;
  used: number;
  level: Exclude<NotifyLevel, 'ok'>;
}

export interface NotifyPlan {
  fires: NotifyFire[];
  nextStates: Record<string, ProviderNotifyState>;
  changed: Set<string>;
}

function usedOf(w: QuotaWindow): number {
  return w.used != null ? w.used : 100 - w.remaining;
}

export function levelFor(used: number): NotifyLevel {
  if (used >= CRITICAL_USED) return 'critical';
  if (used >= LOW_USED) return 'low';
  return 'ok';
}

/**
 * Distinct quota windows of a provider, each with a stable dedup `key`, a
 * human `label`, and used percent.
 *
 * `primary`/`secondary` are frequently ALIASES of a `models` entry (Amp's
 * Free Tier, Codex per-model) — emitting both would double-notify. We collect
 * named windows first, then drop any later window whose (used, resetsAt)
 * signature was already seen, so the friendly model label wins and the alias
 * is dropped. Claude's per-model weekly limits live in `extra.weeklyModels`
 * (never in `p.models`), so they are included explicitly.
 */
function windowsOf(p: ProviderQuota): { key: string; label: string; used: number }[] {
  const raw: { key: string; label: string; w: QuotaWindow }[] = [];
  if (p.models) {
    for (const [name, w] of Object.entries(p.models)) raw.push({ key: `m:${name}`, label: name, w });
  }
  const weekly = getClaudeExtra(p)?.weeklyModels;
  if (weekly) {
    for (const [name, w] of Object.entries(weekly)) raw.push({ key: `w:${name}`, label: `${name} (weekly)`, w });
  }
  if (p.primary) raw.push({ key: 'primary', label: 'primary', w: p.primary });
  if (p.secondary) raw.push({ key: 'secondary', label: 'secondary', w: p.secondary });

  const seen = new Set<string>();
  const out: { key: string; label: string; used: number }[] = [];
  for (const { key, label, w } of raw) {
    const used = usedOf(w);
    const sig = `${Math.round(used)}|${w.resetsAt ?? ''}`;
    if (seen.has(sig)) continue;
    seen.add(sig);
    out.push({ key, label, used });
  }
  return out;
}

/**
 * Pure decision: given current quotas and the previous per-provider state,
 * return which notifications to fire and the next state. Notify only when a
 * window ESCALATES to a higher level than last recorded; re-arm (without
 * notifying) when it recovers, so a later crossing fires again.
 */
export function planNotifications(quotas: AllQuotas, prevStates: Record<string, ProviderNotifyState>): NotifyPlan {
  const fires: NotifyFire[] = [];
  const nextStates: Record<string, ProviderNotifyState> = {};
  const changed = new Set<string>();

  for (const p of quotas.providers) {
    if (!p.available) continue;

    const prev = prevStates[p.provider]?.windows ?? {};
    const next: Record<string, NotifyLevel> = {};

    for (const { key, label, used } of windowsOf(p)) {
      const current = levelFor(used);
      // Sanitize: a stale/hand-edited state value that isn't a known level
      // must read as 'ok' so a real crossing can still fire.
      const stored = prev[key];
      const previous: NotifyLevel = stored === 'low' || stored === 'critical' ? stored : 'ok';

      if (RANK[current] > RANK[previous]) {
        // current outranks previous (>= 'ok'), so it is 'low' or 'critical'.
        fires.push({
          provider: p.provider,
          displayName: p.displayName,
          label,
          used,
          level: current as Exclude<NotifyLevel, 'ok'>,
        });
        next[key] = current;
        changed.add(p.provider);
      } else if (current !== previous) {
        // recovered (or sanitized down) → re-arm without notifying.
        next[key] = current;
        changed.add(p.provider);
      } else if (previous !== 'ok') {
        // unchanged at low/critical → keep persisting the level.
        next[key] = previous;
      }
    }

    nextStates[p.provider] = { windows: next };
  }

  return { fires, nextStates, changed };
}

function statePath(provider: string): string {
  return join(CONFIG.paths.cache, `notify-${provider}.json`);
}

async function readState(provider: string): Promise<ProviderNotifyState> {
  try {
    const file = Bun.file(statePath(provider));
    if (await file.exists()) {
      const data = (await file.json()) as ProviderNotifyState;
      if (data && typeof data === 'object' && data.windows) return data;
    }
  } catch (error) {
    logger.debug('notify state read failed', { provider, error });
  }
  return { windows: {} };
}

async function writeState(provider: string, state: ProviderNotifyState): Promise<void> {
  try {
    const { mkdir } = await import('node:fs/promises');
    await mkdir(CONFIG.paths.cache, { recursive: true });
    const path = statePath(provider);
    const tmp = `${path}.${process.pid}.tmp`;
    await Bun.write(tmp, JSON.stringify(state));
    await rename(tmp, path);
  } catch (error) {
    logger.debug('notify state write failed', { provider, error });
  }
}

function fireNotification(fire: NotifyFire): void {
  const left = Math.max(0, 100 - Math.round(fire.used));
  const title = `${fire.displayName} quota ${fire.level === 'critical' ? 'critical' : 'low'}`;
  const body = `${fire.label}: ${Math.round(fire.used)}% used (${left}% left)`;
  try {
    const proc = spawn(
      'notify-send',
      ['--app-name=agent-bar', `--urgency=${fire.level === 'critical' ? 'critical' : 'normal'}`, title, body],
      { stdio: 'ignore' },
    );
    // notify-send not installed → swallow the spawn error, never break the bar.
    proc.on('error', () => {});
    proc.unref();
  } catch (error) {
    logger.debug('notify-send spawn failed', { error });
  }
}

/**
 * Check quota windows and emit desktop notifications on threshold crossings.
 * Best-effort: reads/writes per-provider dedup state and never throws.
 */
export async function checkAndNotify(quotas: AllQuotas): Promise<void> {
  // If notify-send is unavailable, do nothing — and crucially do NOT persist
  // state, so a crossing fires once the user installs it (instead of being
  // permanently marked as already-notified).
  if (!Bun.which('notify-send')) return;

  const available = quotas.providers.filter((p) => p.available);
  const prevStates: Record<string, ProviderNotifyState> = {};
  await Promise.all(
    available.map(async (p) => {
      prevStates[p.provider] = await readState(p.provider);
    }),
  );

  const plan = planNotifications(quotas, prevStates);

  for (const fire of plan.fires) {
    fireNotification(fire);
  }

  await Promise.all([...plan.changed].map((provider) => writeState(provider, plan.nextStates[provider])));
}

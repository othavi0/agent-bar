import { APP_BASE_CLASS } from '../app-identity';
import { getStatusForPercent } from '../config';
import type { AllQuotas, ProviderQuota } from '../providers/types';
import { type DisplayMode, loadSettingsSync } from '../settings';
import { ONE_DARK } from '../theme';
import { buildAmp as buildAmpLines } from './builders/amp';
import { buildClaude } from './builders/claude';
import { buildCodex as buildCodexLines } from './builders/codex';
import { buildCopilot as buildCopilotLines } from './builders/copilot';
import { buildGeneric } from './builders/generic';
import { TOOLTIP_BORDER } from './builders/shared';
import { HEX_BY_TOKEN, renderPango, span } from './render-pango';
import { colorForDisplay } from './segments';
import { formatPercent, normalizePlanLabel, toWindowDisplay } from './shared';
import { type CodexViewModel, resolveCodexViewModelFrom } from './view-model';

const SETTINGS_CACHE_TTL_MS = 5_000;

let settingsCache: {
  value: ReturnType<typeof loadSettingsSync>;
  expiresAt: number;
} | null = null;

/**
 * Cached settings loader for the Waybar hot path.
 *
 * Waybar invokes `agent-bar` on a tight polling interval (default a few
 * seconds), so reading settings.json from disk every call adds up. `SETTINGS_CACHE_TTL_MS`
 * makes hot runs O(1).
 *
 * Other entry points (refresh, action-right, index) are one-shot per invocation
 * and intentionally use `loadSettingsSync` directly — caching there is YAGNI.
 */
function loadSettingsCached(): ReturnType<typeof loadSettingsSync> {
  const now = Date.now();
  if (settingsCache && settingsCache.expiresAt > now) {
    return settingsCache.value;
  }

  const value = loadSettingsSync();
  settingsCache = {
    value,
    expiresAt: now + SETTINGS_CACHE_TTL_MS,
  };
  return value;
}

/**
 * Cached Codex view-model resolver for the Waybar hot path.
 *
 * Derives the view model via `resolveCodexViewModelFrom`, fed by the 5s-TTL
 * `loadSettingsCached`, so settings are read from disk at most once per cache
 * window regardless of how many times Waybar polls.
 */
function resolveCodexViewModelCached(p: ProviderQuota): CodexViewModel {
  return resolveCodexViewModelFrom(loadSettingsCached(), p);
}

interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
}

function colorFor(display: number | null, mode: DisplayMode): string {
  return HEX_BY_TOKEN[colorForDisplay(display, mode)];
}

function pctColored(display: number | null, mode: DisplayMode): string {
  return span(colorFor(display, mode), formatPercent(display));
}

/**
 * Build Claude tooltip
 */
function buildClaudeTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const planLabel = normalizePlanLabel(p);
  const subtitle = planLabel !== 'Unknown' ? planLabel : undefined;
  const headerTitle = subtitle ? `Claude · ${subtitle}` : 'Claude';

  return renderPango(
    buildClaude(p, {
      mode,
      headerTitle,
      headerWidth: TOOLTIP_BORDER - 4,
      labelColor: 'orange',
      footer: { fetchedAt },
    }),
  );
}

/**
 * Build Codex tooltip
 */
function buildCodexTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const viewModel = resolveCodexViewModelCached(p);
  const planLabel = normalizePlanLabel(p);
  const subtitle = planLabel !== 'Unknown' ? planLabel : undefined;
  const headerTitle = subtitle ? `Codex · ${subtitle}` : 'Codex';

  return renderPango(
    buildCodexLines(p, viewModel, {
      mode,
      headerTitle,
      headerWidth: TOOLTIP_BORDER - 4,
      labelColor: 'green',
      footer: { fetchedAt },
    }),
  );
}

/**
 * Build Copilot tooltip
 */
function buildCopilotTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const headerTitle = p.account ? `Copilot · ${p.account}` : 'Copilot';

  return renderPango(
    buildCopilotLines(p, {
      mode,
      headerTitle,
      headerWidth: TOOLTIP_BORDER - 4,
      labelColor: 'brightBlue',
      footer: { fetchedAt },
      accountInHeader: true,
    }),
  );
}

/**
 * Build Amp tooltip
 */
function buildAmpTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const headerTitle = p.account ? `Amp · ${p.account}` : 'Amp';

  return renderPango(
    buildAmpLines(p, {
      mode,
      headerTitle,
      headerWidth: TOOLTIP_BORDER - 4,
      labelColor: 'magenta',
      ampFreeTierLayout: 'inline',
      footer: { fetchedAt },
      accountInHeader: true,
    }),
  );
}

// ---------------------------------------------------------------------------
// Tooltip builder registry
// ---------------------------------------------------------------------------

type TooltipBuilder = (p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode) => string;

const TOOLTIP_BUILDERS: Record<string, TooltipBuilder> = {
  claude: buildClaudeTooltip,
  codex: buildCodexTooltip,
  copilot: buildCopilotTooltip,
  amp: buildAmpTooltip,
};

function buildProviderTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const builder = TOOLTIP_BUILDERS[p.provider];
  if (builder) return builder(p, fetchedAt, mode);
  const name = p.displayName ?? p.provider;
  return renderPango(
    buildGeneric(p, {
      mode,
      headerTitle: name,
      headerWidth: TOOLTIP_BORDER - 4,
      labelColor: 'text',
      footer: { fetchedAt },
    }),
  );
}

function buildTooltip(quotas: AllQuotas, mode: DisplayMode): string {
  const sections: string[] = [];
  const fetchedAt = quotas.fetchedAt;

  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    sections.push(buildProviderTooltip(p, fetchedAt, mode));
  }

  return sections.join('\n\n');
}

function buildText(quotas: AllQuotas, mode: DisplayMode): string {
  const parts: string[] = [];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const disp = toWindowDisplay(p.primary, mode);
    parts.push(pctColored(disp, mode));
  }

  if (parts.length === 0) return span(ONE_DARK.comment, 'No Providers');
  return parts.join(` ${span(ONE_DARK.comment, '│')} `);
}

function getClass(quotas: AllQuotas): string {
  const classes: string[] = [APP_BASE_CLASS];

  for (const p of quotas.providers) {
    if (!p.available) continue;
    const val = p.primary?.remaining ?? 100;
    const status = getStatusForPercent(val);
    classes.push(`${p.provider}-${status}`);
  }

  return classes.join(' ');
}

export function formatForWaybar(quotas: AllQuotas, mode: DisplayMode = 'remaining'): WaybarOutput {
  return {
    text: buildText(quotas, mode),
    tooltip: buildTooltip(quotas, mode),
    class: getClass(quotas),
  };
}

export function outputWaybar(quotas: AllQuotas, mode: DisplayMode = 'remaining'): void {
  console.log(JSON.stringify(formatForWaybar(quotas, mode)));
}

export function formatProviderForWaybar(quota: ProviderQuota, mode: DisplayMode = 'remaining'): WaybarOutput {
  // Disconnected is a terminal status — class omits health prefix and `mode` only affects tooltip.
  if (!quota.available || quota.error) {
    return {
      text: `<span foreground='${ONE_DARK.red}'>󱘖</span>`,
      tooltip: buildProviderTooltip(quota, undefined, mode),
      class: `${APP_BASE_CLASS}-${quota.provider} disconnected`,
    };
  }

  const rem = quota.primary?.remaining ?? null;
  const disp = toWindowDisplay(quota.primary, mode);
  // class based on health (raw remaining), not display value
  const health = rem ?? 100;
  const status = getStatusForPercent(health);

  return {
    text: pctColored(disp, mode),
    tooltip: buildProviderTooltip(quota, undefined, mode),
    class: `${APP_BASE_CLASS}-${quota.provider} ${status}`,
  };
}

import * as p from '@clack/prompts';
import { loadSettings, saveSettings, type WindowPolicy } from '../settings';
import { providers } from '../providers';
import type { ModelWindows, ProviderQuota, QuotaWindow } from '../providers/types';
import { catppuccin, semantic, colorize } from './colors';

interface ProviderOption {
  id: string;
  name: string;
  modelCount: number;
  planLabel: string;
  quota: ProviderQuota;
}

function classifyWindow(minutes: number | null | undefined): 'fiveHour' | 'sevenDay' | 'other' {
  if (!minutes || minutes <= 0) return 'other';
  if (Math.abs(minutes - 300) <= 90) return 'fiveHour';
  if (Math.abs(minutes - 10080) <= 1440) return 'sevenDay';
  return 'other';
}

function normalizePlanLabel(quota: ProviderQuota): string {
  if (quota.plan?.trim()) return quota.plan;
  const raw = quota.planType?.trim();
  if (!raw) return 'Unknown';

  const key = raw.toLowerCase();
  const map: Record<string, string> = {
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
  return map[key] ?? raw;
}

function getModelWindowsMap(quota: ProviderQuota): Record<string, ModelWindows> {
  const result: Record<string, ModelWindows> = {};

  if (quota.modelsDetailed) {
    for (const [name, windows] of Object.entries(quota.modelsDetailed)) {
      result[name] = windows;
    }
  }

  if (quota.models) {
    for (const [name, window] of Object.entries(quota.models)) {
      if (!result[name]) result[name] = {};
      const kind = classifyWindow(window.windowMinutes);
      if (kind === 'fiveHour' && !result[name].fiveHour) result[name].fiveHour = window;
      else if (kind === 'sevenDay' && !result[name].sevenDay) result[name].sevenDay = window;
      else {
        if (!result[name].other) result[name].other = [];
        result[name].other!.push(window);
      }
    }
  }

  if (Object.keys(result).length === 0 && quota.provider === 'codex' && (quota.primary || quota.secondary)) {
    const fallback: ModelWindows = {};
    if (quota.primary) {
      const kind = classifyWindow(quota.primary.windowMinutes);
      if (kind === 'fiveHour') fallback.fiveHour = quota.primary;
      else if (kind === 'sevenDay') fallback.sevenDay = quota.primary;
      else fallback.other = [quota.primary];
    }
    if (quota.secondary) {
      const kind = classifyWindow(quota.secondary.windowMinutes);
      if (kind === 'fiveHour' && !fallback.fiveHour) fallback.fiveHour = quota.secondary;
      else if (kind === 'sevenDay' && !fallback.sevenDay) fallback.sevenDay = quota.secondary;
      else {
        if (!fallback.other) fallback.other = [];
        fallback.other.push(quota.secondary);
      }
    }
    result['All Models'] = fallback;
  }

  return result;
}

function severity(windows: ModelWindows): number {
  const values = [
    windows.fiveHour?.remaining,
    windows.sevenDay?.remaining,
    ...(windows.other?.map((w) => w.remaining) ?? []),
  ].filter((v): v is number => v !== undefined && v !== null);

  return values.length > 0 ? Math.min(...values) : 101;
}

function formatEta(iso: string | null, remaining: number): string {
  if (remaining === 100) return 'Full';
  if (!iso) return 'N/A';

  const diff = new Date(iso).getTime() - Date.now();
  if (diff <= 0) return '0m';

  const d = Math.floor(diff / 86400000);
  const h = Math.floor((diff % 86400000) / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  return d > 0 ? `${d}d ${h.toString().padStart(2, '0')}h` : `${h}h ${m.toString().padStart(2, '0')}m`;
}

function describeWindow(window: QuotaWindow | undefined): string {
  if (!window) return 'N/A';
  const pct = Math.round(window.remaining);
  return `${pct}% (${formatEta(window.resetsAt, window.remaining)})`;
}

function policyLabel(policy: WindowPolicy): string {
  if (policy === 'five_hour') return 'Only 5h';
  if (policy === 'seven_day') return 'Only 7d';
  return '5h + 7d (N/A fallback)';
}

function buildModelHint(windows: ModelWindows, policy: WindowPolicy): string {
  const fiveHour = `5h: ${describeWindow(windows.fiveHour)}`;
  const sevenDay = `7d: ${describeWindow(windows.sevenDay)}`;
  if (policy === 'five_hour') return fiveHour;
  if (policy === 'seven_day') return sevenDay;
  return `${fiveHour} | ${sevenDay}`;
}

export async function configureModels(): Promise<boolean> {
  const settings = await loadSettings();

  const spinner = p.spinner();
  spinner.start('Checking providers for model data...');

  const providerOptions: ProviderOption[] = [];
  for (const prov of providers) {
    try {
      const quota = await prov.getQuota();
      const modelWindows = getModelWindowsMap(quota);
      const modelCount = Object.keys(modelWindows).length;
      if (modelCount > 0) {
        providerOptions.push({
          id: prov.id,
          name: prov.name,
          modelCount,
          planLabel: normalizePlanLabel(quota),
          quota,
        });
      }
    } catch {
      // Skip providers that fail discovery.
    }
  }

  spinner.stop(colorize(`${providerOptions.length} provider(s) with model-level data`, semantic.good));

  if (providerOptions.length === 0) {
    p.log.warn(colorize('No providers with model-level data found', semantic.warning));
    return false;
  }

  const providerChoice = await p.select({
    message: colorize('Select provider to configure models', semantic.title),
    options: [
      ...providerOptions.map((opt) => ({
        value: opt.id,
        label: colorize(opt.name, catppuccin.text),
        hint: colorize(`${opt.modelCount} models • Plan: ${opt.planLabel}`, semantic.muted),
      })),
      { value: 'back' as const, label: colorize('Back', semantic.muted) },
    ],
  });

  if (p.isCancel(providerChoice) || providerChoice === 'back') return false;

  const selectedProvider = providerOptions.find((opt) => opt.id === providerChoice);
  if (!selectedProvider) return false;

  const providerId = selectedProvider.id;
  const modelWindows = getModelWindowsMap(selectedProvider.quota);
  const entries = Object.entries(modelWindows)
    .sort((a, b) => severity(a[1]) - severity(b[1]) || a[0].localeCompare(b[0]));

  if (entries.length === 0) {
    p.log.warn(colorize('No models available for this provider', semantic.warning));
    return false;
  }

  const currentPolicy = settings.windowPolicy?.[providerId] ?? 'both';
  const policyChoice = await p.select({
    message: colorize('Select window display policy', semantic.title),
    options: [
      {
        value: 'both' as const,
        label: colorize('Always show 5h + 7d', catppuccin.text),
        hint: colorize('Missing windows are shown as N/A', semantic.muted),
      },
      {
        value: 'five_hour' as const,
        label: colorize('Show only 5h', catppuccin.text),
        hint: colorize('Hide 7d column in tooltips', semantic.muted),
      },
      {
        value: 'seven_day' as const,
        label: colorize('Show only 7d', catppuccin.text),
        hint: colorize('Hide 5h column in tooltips', semantic.muted),
      },
    ],
    initialValue: currentPolicy,
  });

  if (p.isCancel(policyChoice)) return false;
  const selectedPolicy = policyChoice as WindowPolicy;

  const modelNames = entries.map(([name]) => name);
  const currentSelection = settings.models?.[providerId] ?? [];
  const initialValues = currentSelection.length > 0
    ? currentSelection.filter((name) => modelNames.includes(name))
    : modelNames;
  const preview = entries[0];

  p.note(
    [
      colorize('Space', semantic.highlight) + ' toggle  ' +
      colorize('Enter', semantic.highlight) + ' confirm  ' +
      colorize('q', semantic.highlight) + ' back',
      '',
      `Plan: ${selectedProvider.planLabel}`,
      `Window policy: ${policyLabel(selectedPolicy)}`,
      preview
        ? `Preview (${preview[0]}): ${buildModelHint(preview[1], selectedPolicy)}`
        : 'Preview: N/A',
      '',
      'Selected models appear in the tooltip on hover.',
      'Deselected models are hidden.',
    ].join('\n'),
    colorize(`${providerId} Models`, semantic.title)
  );

  const result = await p.multiselect({
    message: colorize('Select models to show in tooltip', semantic.title),
    options: entries.map(([name, windows]) => ({
      value: name,
      label: colorize(name, catppuccin.text),
      hint: colorize(buildModelHint(windows, selectedPolicy), semantic.muted),
    })),
    initialValues,
    required: false,
  });

  if (p.isCancel(result)) return false;

  const selected = result as string[];

  if (!settings.models) settings.models = {};
  if (!settings.windowPolicy) settings.windowPolicy = {};
  settings.models[providerId] = selected.length === modelNames.length ? [] : selected;
  settings.windowPolicy[providerId] = selectedPolicy;

  await saveSettings(settings);
  p.log.success(
    colorize(
      `Model configuration saved (${selectedProvider.planLabel}, ${policyLabel(selectedPolicy)})`,
      semantic.good
    )
  );
  return true;
}

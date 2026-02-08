import * as p from '@clack/prompts';
import { loadSettings, saveSettings } from '../settings';
import { providers, getProvider } from '../providers';
import { catppuccin, semantic, colorize } from './colors';

export async function configureModels(): Promise<boolean> {
  const settings = await loadSettings();

  // Discover which providers have models by checking their current quota
  const spinner = p.spinner();
  spinner.start('Checking providers for model data...');
  
  const providerOptions: Array<{id: string, name: string, modelCount: number}> = [];
  for (const prov of providers) {
    try {
      const quota = await prov.getQuota();
      if (quota.models && Object.keys(quota.models).length > 0) {
        providerOptions.push({ id: prov.id, name: prov.name, modelCount: Object.keys(quota.models).length });
      }
    } catch { /* skip */ }
  }
  
  spinner.stop(colorize(`${providerOptions.length} provider(s) with models`, semantic.good));
  
  if (providerOptions.length === 0) {
    p.log.warn(colorize('No providers with model-level data found', semantic.warning));
    return false;
  }

  const providerChoice = await p.select({
    message: colorize('Select provider to configure models', semantic.title),
    options: [
      ...providerOptions.map(opt => ({
        value: opt.id,
        label: colorize(opt.name, catppuccin.text),
        hint: colorize(`${opt.modelCount} models`, semantic.muted),
      })),
      { value: 'back' as const, label: colorize('Back', semantic.muted) },
    ],
  });

  if (p.isCancel(providerChoice) || providerChoice === 'back') return false;

  const providerId = providerChoice as string;
  const provider = getProvider(providerId);
  if (!provider) return false;

  const quota = await provider.getQuota();

  if (!quota.models || Object.keys(quota.models).length === 0) {
    p.log.warn(colorize('No models available for this provider', semantic.warning));
    return false;
  }

  const modelNames = Object.keys(quota.models);
  const currentSelection = settings.models?.[providerId] ?? [];
  const initialValues = currentSelection.length > 0 ? currentSelection : modelNames;

  p.note(
    [
      colorize('Space', semantic.highlight) + ' toggle  ' +
      colorize('Enter', semantic.highlight) + ' confirm  ' +
      colorize('q', semantic.highlight) + ' back',
      '',
      'Selected models appear in the tooltip on hover.',
      'Deselected models are hidden.',
    ].join('\n'),
    colorize(`${providerId} Models`, semantic.title)
  );

  const result = await p.multiselect({
    message: colorize('Select models to show in tooltip', semantic.title),
    options: modelNames.map(name => {
      const window = quota.models![name];
      const pct = window.remaining;
      const hint = `${pct}% remaining`;
      return {
        value: name,
        label: colorize(name, catppuccin.text),
        hint: colorize(hint, semantic.muted),
      };
    }),
    initialValues,
    required: false,
  });

  if (p.isCancel(result)) return false;

  const selected = result as string[];

  if (!settings.models) settings.models = {};
  settings.models[providerId] = selected.length === modelNames.length ? [] : selected;

  await saveSettings(settings);
  p.log.success(colorize('Model configuration saved', semantic.good));
  return true;
}

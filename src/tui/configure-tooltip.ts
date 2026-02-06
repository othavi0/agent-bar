import * as p from '@clack/prompts';
import { loadSettings, saveSettings } from '../settings';
import { providers } from '../providers';
import { catppuccin, semantic, colorize } from './colors';

export async function configureTooltip(): Promise<boolean> {
  const settings = await loadSettings();

  // Get available providers
  const availableProviders = await Promise.all(
    providers.map(async (provider) => ({
      id: provider.id,
      name: provider.name,
      available: await provider.isAvailable(),
    }))
  );

  const options = availableProviders.map((prov) => ({
    value: prov.id,
    label: prov.available 
      ? colorize(prov.name, catppuccin.green)
      : colorize(prov.name, catppuccin.text) + colorize(' (not logged in)', semantic.muted),
    hint: prov.available ? undefined : 'credentials not found',
  }));

  // Tips box
  p.note(
    [
      colorize('Space', semantic.highlight) + ' toggle  ' +
      colorize('Enter', semantic.highlight) + ' confirm  ' +
      colorize('q', semantic.highlight) + ' back',
    ].join('\n'),
    colorize('Tooltip Display', semantic.title)
  );

  const result = await p.multiselect({
    message: colorize('Select providers to show on hover', semantic.title),
    options,
    initialValues: settings.tooltip.providers.filter(id => 
      availableProviders.some(p => p.id === id)
    ),
    required: false,
  });

  if (p.isCancel(result)) {
    return false;
  }

  settings.tooltip.providers = result as string[];

  // Additional tooltip options
  const showWeekly = await p.confirm({
    message: colorize('Show weekly quotas?', catppuccin.text),
    initialValue: settings.tooltip.showWeekly,
  });

  if (p.isCancel(showWeekly)) {
    return false;
  }

  const showResetTime = await p.confirm({
    message: colorize('Show reset time countdown?', catppuccin.text),
    initialValue: settings.tooltip.showResetTime,
  });

  if (p.isCancel(showResetTime)) {
    return false;
  }

  settings.tooltip.showWeekly = showWeekly as boolean;
  settings.tooltip.showResetTime = showResetTime as boolean;
  await saveSettings(settings);

  p.log.success(colorize('Tooltip configuration saved', semantic.good));
  return true;
}

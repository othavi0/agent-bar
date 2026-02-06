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

  const options = availableProviders.map((p) => ({
    value: p.id,
    label: p.available ? p.name : colorize(`${p.name} (not logged in)`, semantic.muted),
    hint: p.available ? undefined : 'credentials not found',
  }));

  console.log('');
  console.log(colorize('  Tip: Space to toggle, Enter to confirm, q to go back', semantic.subtitle));
  console.log('');

  const result = await p.multiselect({
    message: 'Select providers to show in tooltip (on hover)',
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
  await saveSettings(settings);

  // Additional tooltip options
  const showWeekly = await p.confirm({
    message: 'Show weekly quotas?',
    initialValue: settings.tooltip.showWeekly,
  });

  if (p.isCancel(showWeekly)) {
    return false;
  }

  const showResetTime = await p.confirm({
    message: 'Show reset time countdown?',
    initialValue: settings.tooltip.showResetTime,
  });

  if (p.isCancel(showResetTime)) {
    return false;
  }

  settings.tooltip.showWeekly = showWeekly as boolean;
  settings.tooltip.showResetTime = showResetTime as boolean;
  await saveSettings(settings);

  p.log.success('Tooltip configuration saved');
  return true;
}

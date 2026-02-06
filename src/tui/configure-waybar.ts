import * as p from '@clack/prompts';
import { loadSettings, saveSettings } from '../settings';
import { providers } from '../providers';
import { catppuccin, semantic, colorize } from './colors';

export async function configureWaybar(): Promise<boolean> {
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
    colorize('Waybar Display', semantic.title)
  );

  const result = await p.multiselect({
    message: colorize('Select providers to show in Waybar', semantic.title),
    options,
    initialValues: settings.waybar.providers.filter(id => 
      availableProviders.some(p => p.id === id)
    ),
    required: false,
  });

  if (p.isCancel(result)) {
    return false;
  }

  settings.waybar.providers = result as string[];
  await saveSettings(settings);

  p.log.success(colorize('Waybar configuration saved', semantic.good));
  return true;
}

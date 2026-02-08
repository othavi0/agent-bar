/**
 * qbar action-right <provider>
 *
 * Used by Waybar right-click.
 * - If provider is disconnected/expired: start login flow.
 * - Else: refresh that provider and show result.
 */

import * as p from '@clack/prompts';
import { getProvider } from './providers';
import { loadSettings, saveSettings } from './settings';
import { getQuotaFor } from './providers';
import { colorize, semantic } from './tui/colors';

async function activateProvider(providerId: string): Promise<void> {
  const settings = await loadSettings();

  if (!settings.waybar.providers.includes(providerId)) {
    settings.waybar.providers.push(providerId);
  }
  await saveSettings(settings);
}

async function waitEnter(): Promise<void> {
  const { createInterface } = await import('node:readline');
  p.log.info(colorize('Press Enter to close...', semantic.subtitle));
  return new Promise<void>((resolve) => {
    const rl = createInterface({ input: process.stdin });
    rl.once('line', () => {
      rl.close();
      resolve();
    });
  });
}

export async function handleActionRight(providerId: string): Promise<void> {
  if (!providerId) {
    console.error('Usage: qbar action-right <provider>');
    process.exit(1);
  }

  const provider = getProvider(providerId);
  if (!provider) {
    console.error(`Unknown provider: ${providerId}`);
    await waitEnter();
    return;
  }

  const available = await provider.isAvailable();

  // If not available: go straight to login.
  if (!available) {
    const { loginSingleProvider } = await import('./tui/login-single');
    await loginSingleProvider(providerId);
    await activateProvider(providerId);
    return;
  }

  // If available, check if provider is effectively disconnected (expired token, etc.)
  const quota = await provider.getQuota();
  const looksDisconnected = !!quota.error && /expired|not logged in|login again|please login/i.test(quota.error);

  if (looksDisconnected) {
    const { loginSingleProvider } = await import('./tui/login-single');
    await loginSingleProvider(providerId);
    await activateProvider(providerId);
    return;
  }

  // Otherwise: refresh and show status
  p.intro(colorize(`Refreshing ${provider.name}...`, semantic.accent));

  const fresh = await provider.getQuota();

  if (fresh.error) {
    p.log.error(colorize(`⚠️ ${fresh.error}`, semantic.danger));
  } else if (fresh.primary) {
    const pct = fresh.primary.remaining ?? 0;
    const color = pct >= 60 ? semantic.good : pct >= 30 ? semantic.warning : semantic.danger;
    p.log.success(colorize(`${provider.name}: ${pct}% remaining`, color));
  } else {
    p.log.success(colorize(`${provider.name}: refreshed`, semantic.good));
  }

  await waitEnter();
}

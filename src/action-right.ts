#!/usr/bin/env bun

/**
 * qbar action-right <provider>
 *
 * Used by Waybar right-click.
 * - If provider is disconnected: start login flow.
 * - Else: refresh that provider.
 */

import { getProvider } from './providers';
import { loadSettings, saveSettings } from './settings';

const providerId = process.argv[2];

if (!providerId) {
  console.error('Usage: qbar action-right <provider>');
  process.exit(1);
}

const provider = getProvider(providerId);
if (!provider) {
  console.error(`Unknown provider: ${providerId}`);
  process.exit(1);
}

// help TS understand provider is defined after the guard
const prov = provider;

async function activateProvider(providerId: string): Promise<void> {
  const settings = await loadSettings();

  if (!settings.waybar.providers.includes(providerId)) {
    settings.waybar.providers.push(providerId);
  }
  if (!settings.tooltip.providers.includes(providerId)) {
    settings.tooltip.providers.push(providerId);
  }

  await saveSettings(settings);
}

async function main() {
  const available = await prov.isAvailable();

  // If not available: go straight to login.
  if (!available) {
    const { loginSingleProvider } = await import('./tui/login-single');
    await loginSingleProvider(providerId);
    await activateProvider(providerId);
    return;
  }

  // If available, check if provider is effectively disconnected (expired token, etc.)
  const quota = await prov.getQuota();
  const looksDisconnected = !!quota.error && /expired|not logged in|login again|please login/i.test(quota.error);

  if (looksDisconnected) {
    const { loginSingleProvider } = await import('./tui/login-single');
    await loginSingleProvider(providerId);
    await activateProvider(providerId);
    return;
  }

  // Otherwise: refresh
  const proc = Bun.spawn(['bun', 'src/refresh.ts', providerId], {
    stdin: 'inherit',
    stdout: 'inherit',
    stderr: 'inherit',
  });
  await proc.exited;
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});

/**
 * qbar action-right <provider>
 *
 * Used by Waybar right-click.
 * - If provider is disconnected/expired: start login flow.
 * - Else: refresh that provider and show full status in terminal.
 */

import * as p from '@clack/prompts';
import { getProvider, getQuotaFor } from './providers';
import { outputTerminal } from './formatters/terminal';
import { colorize, semantic } from './tui/colors';

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
    return;
  }

  // If available, check if provider is effectively disconnected (expired token, etc.)
  const quota = await provider.getQuota();
  const baseDisconnect = /expired|not logged in|login again|please login/i;
  const codexDisconnect = /no session data|no rate limit data|auth|token/i;
  const looksDisconnected = !!quota.error && (
    baseDisconnect.test(quota.error) ||
    (providerId === 'codex' && codexDisconnect.test(quota.error))
  );

  if (looksDisconnected) {
    const { loginSingleProvider } = await import('./tui/login-single');
    await loginSingleProvider(providerId);
    return;
  }

  // Otherwise: refresh and show full terminal output
  p.intro(colorize(`Refreshing ${provider.name}...`, semantic.accent));

  // Force refresh on right-click (ignore TTL cache).
  try {
    const { cache } = await import('./cache');
    if (providerId === 'codex') await cache.invalidate('codex-quota');
    if (providerId === 'claude') await cache.invalidate('claude-usage');
    // Antigravity cache keys are per-account; we don't nuke them here to avoid heavy I/O.
  } catch {
    // ignore
  }

  const fresh = await getQuotaFor(providerId);
  if (fresh) {
    outputTerminal({
      providers: [fresh],
      fetchedAt: new Date().toISOString(),
    });
  } else {
    p.log.error(colorize(`Failed to fetch ${provider.name} quota`, semantic.danger));
  }

  await waitEnter();
}

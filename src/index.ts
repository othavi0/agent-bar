#!/usr/bin/env bun

import { APP_HIDDEN_CLASS } from './app-identity';
import { cache } from './cache';
import { parseArgs, showHelp } from './cli';
import { outputTerminal } from './formatters/terminal';
import { formatProviderForWaybar, outputWaybar } from './formatters/waybar';
import { logger } from './logger';
import { getAllQuotas, getProvider, getQuotaFor, getRegisteredProviderIds } from './providers';
import type { AllQuotas } from './providers/types';
import { loadSettings } from './settings';
import { runTui } from './tui';
import {
  exportWaybarCss,
  exportWaybarModules,
  getDefaultWaybarAssetPaths,
  installWaybarAssets,
  type WaybarProviderId,
} from './waybar-contract';

// Graceful shutdown
process.on('SIGTERM', () => process.exit(0));
process.on('SIGINT', () => process.exit(0));

async function main() {
  const args = process.argv.slice(2);
  const options = parseArgs(args);

  // Setup logging
  if (options.verbose) {
    logger.setLevel('debug');
  } else {
    logger.setSilent(true);
  }

  // Handle help
  if (options.command === 'help') {
    showHelp();
    process.exit(0);
  }

  // Handle version
  if (options.command === 'version') {
    const { default: pkg } = await import('../package.json');
    console.log(pkg.version);
    process.exit(0);
  }

  // Handle menu
  if (options.command === 'menu') {
    await runTui();
    process.exit(0);
  }

  // Handle action-right (waybar right-click)
  if (options.command === 'action-right') {
    const { handleActionRight } = await import('./action-right');
    await handleActionRight(options.provider ?? '');
    process.exit(0);
  }

  // Handle setup
  if (options.command === 'setup') {
    const { main: setupMain } = await import('./setup');
    await setupMain();
    process.exit(0);
  }

  if (options.command === 'assets-install') {
    const defaults = getDefaultWaybarAssetPaths();
    const result = installWaybarAssets({
      waybarDir: options.waybarDir ?? defaults.waybarDir,
      scriptsDir: options.scriptsDir ?? defaults.scriptsDir,
    });
    console.log(JSON.stringify(result));
    process.exit(0);
  }

  if (options.command === 'export-waybar-modules') {
    const defaults = getDefaultWaybarAssetPaths();
    const settings = await loadSettings();
    console.log(
      JSON.stringify(
        exportWaybarModules(
          {
            appBin: options.appBin ?? defaults.appBin,
            terminalScript: options.terminalScript ?? defaults.terminalScript,
          },
          settings.waybar.providerOrder as WaybarProviderId[],
        ),
        null,
        2,
      ),
    );
    process.exit(0);
  }

  if (options.command === 'export-waybar-css') {
    const defaults = getDefaultWaybarAssetPaths();
    const settings = await loadSettings();
    console.log(
      JSON.stringify(
        exportWaybarCss({
          iconsDir: options.iconsDir ?? defaults.iconsDir,
          providerOrder: settings.waybar.providerOrder as WaybarProviderId[],
          separators: settings.waybar.separators,
        }),
        null,
        2,
      ),
    );
    process.exit(0);
  }

  // Handle doctor
  if (options.command === 'doctor') {
    const { main: doctorMain } = await import('./doctor');
    await doctorMain({ dryRun: options.dryRun ?? false, yes: options.yes ?? false });
    process.exit(0);
  }

  // Handle update
  if (options.command === 'update') {
    const { main: updateMain } = await import('./update');
    await updateMain();
    process.exit(0);
  }

  // Handle uninstall
  if (options.command === 'uninstall') {
    const { main: uninstallMain } = await import('./uninstall');
    await uninstallMain();
    process.exit(0);
  }

  if (options.command === 'remove') {
    const { main: removeMain } = await import('./remove');
    await removeMain();
    process.exit(0);
  }

  // Handle cache refresh
  if (options.refresh) {
    const toInvalidate = options.provider ? [options.provider] : getRegisteredProviderIds();

    for (const id of toInvalidate) {
      const prov = getProvider(id);
      if (prov) await cache.invalidate(prov.cacheKey);
    }
    logger.info('Cache invalidated');
  }

  if (options.watch) {
    const { startWatch } = await import('./watch');
    await startWatch({ provider: options.provider, intervalMs: options.intervalSeconds * 1000 });
    return;
  }

  // Load settings
  const settings = await loadSettings();

  // Fetch quotas
  let quotas: AllQuotas;

  if (options.provider) {
    // Waybar: provider disabled in settings → hidden module. (json mode bypasses this gate.)
    if (options.format !== 'json' && !settings.waybar.providers.includes(options.provider)) {
      console.log(JSON.stringify({ text: '', tooltip: '', class: APP_HIDDEN_CLASS }));
      process.exit(0);
    }

    const quota = await getQuotaFor(options.provider);
    if (!quota) {
      logger.error(`Unknown provider: ${options.provider}`);
      process.exit(1);
    }
    quotas = {
      providers: [quota],
      fetchedAt: new Date().toISOString(),
    };
  } else {
    quotas = await getAllQuotas();

    // Filter by settings for waybar output
    if (options.command === 'waybar' && options.format !== 'json') {
      quotas.providers = quotas.providers.filter((p) => settings.waybar.providers.includes(p.provider));
    }
  }

  if (options.format === 'json') {
    const { toJsonOutput } = await import('./formatters/json');
    console.log(JSON.stringify(toJsonOutput(quotas)));
    process.exit(0);
  }

  const mode = settings.waybar.displayMode;

  // Output
  switch (options.command) {
    case 'terminal':
    case 'status':
      outputTerminal(quotas, mode);
      break;
    default:
      // If running in interactive terminal without explicit command, show help
      if (process.stdout.isTTY && args.length === 0) {
        showHelp();
        break;
      }

      // If single provider requested, use individual format for separate modules
      if (options.provider && quotas.providers.length === 1) {
        console.log(JSON.stringify(formatProviderForWaybar(quotas.providers[0], mode)));
      } else {
        outputWaybar(quotas, mode);
      }

      // Desktop notifications on low/critical quota (best-effort, after output so
      // the bar updates promptly). Waybar path only — not terminal/json/watch.
      if (settings.notify?.enabled) {
        const { checkAndNotify } = await import('./notify');
        await checkAndNotify(quotas);
      }
      break;
  }
}

main().catch((error) => {
  logger.error('Fatal error', { error });
  process.exit(1);
});

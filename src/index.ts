#!/usr/bin/env bun

import { parseArgs, showHelp } from './cli';
import { logger } from './logger';
import { cache } from './cache';
import { loadSettings } from './settings';
import { getAllQuotas, getQuotaFor, providers } from './providers';
import { outputWaybar, formatForWaybar } from './formatters/waybar';
import { outputTerminal } from './formatters/terminal';
import { runTui } from './tui';
import type { AllQuotas } from './providers/types';

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

  // Handle menu
  if (options.command === 'menu') {
    await runTui();
    process.exit(0);
  }

  // Handle cache refresh
  if (options.refresh) {
    await cache.invalidate('codex-quota');
    logger.info('Cache invalidated');
  }

  // Load settings
  const settings = await loadSettings();

  // Fetch quotas
  let quotas: AllQuotas;

  if (options.provider) {
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
    if (options.command === 'waybar') {
      quotas.providers = quotas.providers.filter(p => 
        settings.waybar.providers.includes(p.provider)
      );
    }
  }

  // Output
  switch (options.command) {
    case 'terminal':
    case 'status':
      outputTerminal(quotas);
      break;
    
    case 'waybar':
    default:
      outputWaybar(quotas);
      break;
  }
}

main().catch((error) => {
  logger.error('Fatal error', { error });
  process.exit(1);
});

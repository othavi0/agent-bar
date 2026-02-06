#!/usr/bin/env bun

import { parseArgs, showHelp } from './cli';
import { logger } from './logger';
import { cache } from './cache';
import { getAllQuotas, getQuotaFor } from './providers';
import { outputWaybar } from './formatters/waybar';
import { outputTerminal } from './formatters/terminal';
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

  // Show help
  if (options.help) {
    showHelp();
    process.exit(0);
  }

  // Handle cache refresh
  if (options.refresh) {
    await cache.invalidate('codex-quota');
    logger.info('Cache invalidated');
  }

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
  }

  // Output
  if (options.terminal) {
    outputTerminal(quotas);
  } else {
    outputWaybar(quotas);
  }
}

main().catch((error) => {
  logger.error('Fatal error', { error });
  process.exit(1);
});

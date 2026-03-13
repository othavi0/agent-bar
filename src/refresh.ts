#!/usr/bin/env bun

/**
 * qbar refresh - Force refresh with visual spinner
 * Used by right-click on waybar module
 */

import { createSpinner } from './spinner';
import { cache } from './cache';
import { getAllQuotas, getProvider, getQuotaFor, providers } from './providers';
import { outputTerminal } from './formatters/terminal';
import { ANSI } from './theme';

const provider = process.argv[2];

async function refresh() {
  const spinner = createSpinner('Refreshing quotas...');
  spinner.start();

  try {
    // Invalidate cache
    if (provider) {
      const prov = getProvider(provider);
      if (prov) await cache.invalidate(prov.cacheKey);
    } else {
      for (const prov of providers) {
        await cache.invalidate(prov.cacheKey);
      }
    }

    spinner.text = 'Fetching fresh data...';

    if (provider) {
      const quota = await getQuotaFor(provider);
      if (quota) {
        spinner.succeed(`${provider} refreshed!`);
        outputTerminal({ providers: [quota], fetchedAt: new Date().toISOString() });
      } else {
        spinner.fail(`Unknown provider: ${provider}`);
      }
    } else {
      const quotas = await getAllQuotas();
      spinner.succeed('All providers refreshed!');
      outputTerminal(quotas);
    }
    
  } catch (error) {
    spinner.fail('Refresh failed');
    console.error(error);
  }
  
  // Signal waybar to update with new data
  Bun.spawn(['pkill', '-SIGUSR2', 'waybar']);

  // Auto-close after showing results
  console.log(`\n${ANSI.dim}(closing in 3s or press Enter...)${ANSI.reset}`);
  await Promise.race([
    Bun.sleep(3000),
    new Promise<void>((resolve) => {
      try {
        const { createInterface } = require('node:readline');
        const rl = createInterface({ input: process.stdin });
        rl.once('line', () => {
          rl.close();
          resolve();
        });
      } catch {
        // ignore if stdin unavailable
      }
    }),
  ]);
}

refresh();

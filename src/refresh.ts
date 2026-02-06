#!/usr/bin/env bun

/**
 * qbar refresh - Force refresh with visual spinner
 * Used by right-click on waybar module
 */

import ora from 'ora';
import { cache } from './cache';
import { getAllQuotas, getQuotaFor } from './providers';
import { outputTerminal } from './formatters/terminal';

const provider = process.argv[2];

async function refresh() {
  const spinner = ora({
    text: 'Refreshing quotas...',
    spinner: 'dots',
    color: 'cyan',
  }).start();

  try {
    // Invalidate cache
    await cache.invalidate('claude-usage');
    await cache.invalidate('codex-quota');
    await cache.invalidate('antigravity-quota');
    
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
  
  // Simple wait - no stdin tricks
  console.log('\n\x1b[2m(closing in 5s...)\x1b[0m');
  await Bun.sleep(5000);
}

refresh();

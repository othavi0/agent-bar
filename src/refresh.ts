#!/usr/bin/env bun

/**
 * qbar refresh - Force refresh with visual spinner
 * Used by right-click on waybar module
 */

import ora from 'ora';
import { cache } from './cache';
import { getAllQuotas, getQuotaFor } from './providers';
import { formatProviderForWaybar } from './formatters/waybar';
import { outputTerminal } from './formatters/terminal';

const provider = process.argv[2];

// Wait for user input or timeout
async function waitForExit(seconds: number = 3): Promise<void> {
  console.log(`\n\x1b[2m(closing in ${seconds}s or press Enter)\x1b[0m`);
  
  return new Promise((resolve) => {
    const timeout = setTimeout(resolve, seconds * 1000);
    
    if (process.stdin.isTTY) {
      process.stdin.setRawMode(true);
      process.stdin.resume();
      process.stdin.once('data', () => {
        clearTimeout(timeout);
        process.stdin.setRawMode(false);
        resolve();
      });
    }
  });
}

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
        // Show terminal-friendly output, not JSON
        outputTerminal({ providers: [quota], fetchedAt: new Date().toISOString() });
      } else {
        spinner.fail(`Unknown provider: ${provider}`);
      }
    } else {
      const quotas = await getAllQuotas();
      spinner.succeed('All providers refreshed!');
      outputTerminal(quotas);
    }
    
    // Wait before closing
    await waitForExit(3);
    
  } catch (error) {
    spinner.fail('Refresh failed');
    console.error(error);
    await waitForExit(5);
    process.exit(1);
  }
}

refresh();

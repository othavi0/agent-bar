#!/usr/bin/env bun

/**
 * qbar refresh - Force refresh with visual spinner
 * Used by right-click on waybar module
 */

import ora from 'ora';
import { cache } from './cache';
import { getAllQuotas, getQuotaFor } from './providers';
import { outputWaybar, formatProviderForWaybar } from './formatters/waybar';

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
        console.log(JSON.stringify(formatProviderForWaybar(quota)));
      } else {
        spinner.fail(`Unknown provider: ${provider}`);
      }
    } else {
      const quotas = await getAllQuotas();
      spinner.succeed('All providers refreshed!');
      outputWaybar(quotas);
    }
  } catch (error) {
    spinner.fail('Refresh failed');
    console.error(error);
    process.exit(1);
  }
}

refresh();

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

    // Antigravity cache keys are per-account (antigravity-quota-<email>). Nuke them all.
    try {
      const { readdirSync, unlinkSync } = await import('node:fs');
      const { join } = await import('node:path');

      const dir = (await import('./config')).CONFIG.paths.cache;
      for (const name of readdirSync(dir)) {
        if (name.startsWith('antigravity-quota-') && name.endsWith('.json')) {
          try { unlinkSync(join(dir, name)); } catch {}
        }
      }
    } catch {
      // ignore
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
  console.log('\n\x1b[2m(closing in 3s or press Enter...)\x1b[0m');
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

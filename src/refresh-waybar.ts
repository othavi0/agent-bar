#!/usr/bin/env bun

/**
 * qbar refresh-waybar - Refresh with loading animation for waybar
 * 
 * Flow:
 * 1. Set loading state file
 * 2. Signal waybar to update (shows spinner via CSS)
 * 3. Fetch fresh data
 * 4. Remove loading state
 * 5. Signal waybar again (shows real data)
 */

import { cache } from './cache';
import { getQuotaFor } from './providers';
import { CONFIG } from './config';

const provider = process.argv[2];

if (!provider) {
  console.error('Usage: refresh-waybar <provider>');
  process.exit(1);
}

const loadingFile = `${CONFIG.paths.cache}/.loading-${provider}`;

async function refresh() {
  try {
    // 1. Create loading state file
    await Bun.write(loadingFile, Date.now().toString());
    
    // 2. Signal waybar to show loading state
    Bun.spawn(['pkill', '-SIGUSR2', 'waybar']);
    
    // 3. Invalidate cache
    await cache.invalidate('claude-usage');
    await cache.invalidate('codex-quota');
    await cache.invalidate(`antigravity-quota`);
    
    // 4. Fetch fresh data
    await getQuotaFor(provider);
    
    // 5. Remove loading state
    const { unlinkSync } = await import('node:fs');
    try { unlinkSync(loadingFile); } catch {}
    
    // 6. Signal waybar to show real data
    Bun.spawn(['pkill', '-SIGUSR2', 'waybar']);
    
  } catch (error) {
    // Clean up on error
    const { unlinkSync } = await import('node:fs');
    try { unlinkSync(loadingFile); } catch {}
    throw error;
  }
}

refresh().catch(console.error);

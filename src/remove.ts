#!/usr/bin/env bun

import { APP_NAME } from './app-identity';
import { runUninstall } from './uninstall';

export async function main() {
  await runUninstall({ force: true, title: `${APP_NAME} remove` });
}

if (import.meta.main) {
  main().catch((error) => {
    console.error('Remove failed:', error);
    process.exit(1);
  });
}

#!/usr/bin/env bun

/**
 * agent-bar menu - Entry point for the interactive TUI
 */

import { runTui } from './tui';

runTui().catch((error) => {
  console.error('TUI Error:', error);
  process.exit(1);
});

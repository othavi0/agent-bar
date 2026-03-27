#!/usr/bin/env bun

/**
 * agent-bar-omarchy menu - Entry point for the interactive TUI
 */

import { runTui } from './tui';

runTui().catch((error) => {
  console.error('TUI Error:', error);
  process.exit(1);
});

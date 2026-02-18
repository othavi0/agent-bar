#!/usr/bin/env bun

import { cache } from "./cache";
import { parseArgs, showHelp } from "./cli";
import { outputTerminal } from "./formatters/terminal";
import { formatProviderForWaybar, outputWaybar } from "./formatters/waybar";
import { logger } from "./logger";
import { getAllQuotas, getQuotaFor } from "./providers";
import type { AllQuotas } from "./providers/types";
import { loadSettings } from "./settings";
import { runTui } from "./tui";

async function main() {
  const args = process.argv.slice(2);
  const options = parseArgs(args);

  // Setup logging
  if (options.verbose) {
    logger.setLevel("debug");
  } else {
    logger.setSilent(true);
  }

  // Handle help
  if (options.command === "help") {
    showHelp();
    process.exit(0);
  }

  // Handle menu
  if (options.command === "menu") {
    await runTui();
    process.exit(0);
  }

  // Handle action-right (waybar right-click)
  if (options.command === "action-right") {
    const { handleActionRight } = await import("./action-right");
    await handleActionRight(options.provider ?? "");
    process.exit(0);
  }

  // Handle setup
  if (options.command === "setup") {
    const { main: setupMain } = await import("./setup");
    await setupMain();
    process.exit(0);
  }

  // Handle update
  if (options.command === "update") {
    const { main: updateMain } = await import("./update");
    await updateMain();
    process.exit(0);
  }

  // Handle uninstall
  if (options.command === "uninstall") {
    const { main: uninstallMain } = await import("./uninstall");
    await uninstallMain();
    process.exit(0);
  }

  // Handle cache refresh
  if (options.refresh) {
    await cache.invalidate("codex-quota");
    logger.info("Cache invalidated");
  }

  // Load settings
  const settings = await loadSettings();

  // Fetch quotas
  let quotas: AllQuotas;

  if (options.provider) {
    // If provider is disabled in waybar settings, output empty (hidden module)
    if (!settings.waybar.providers.includes(options.provider)) {
      console.log(
        JSON.stringify({ text: "", tooltip: "", class: "qbar-hidden" }),
      );
      process.exit(0);
    }

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

    // Filter by settings for waybar output
    if (options.command === "waybar") {
      quotas.providers = quotas.providers.filter((p) =>
        settings.waybar.providers.includes(p.provider),
      );
    }
  }

  // Output
  switch (options.command) {
    case "terminal":
    case "status":
      outputTerminal(quotas);
      break;

    case "waybar":
    default:
      // If running in interactive terminal without explicit command, show help
      if (process.stdout.isTTY && args.length === 0) {
        showHelp();
        break;
      }

      // If single provider requested, use individual format for separate modules
      if (options.provider && quotas.providers.length === 1) {
        console.log(
          JSON.stringify(formatProviderForWaybar(quotas.providers[0])),
        );
      } else {
        outputWaybar(quotas);
      }
      break;
  }
}

main().catch((error) => {
  logger.error("Fatal error", { error });
  process.exit(1);
});

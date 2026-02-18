#!/usr/bin/env bun

/**
 * qbar update - Update qbar to latest version
 */

import * as p from "@clack/prompts";
import { join } from "node:path";
import { catppuccin, colorize, semantic } from "./tui/colors";

// Get the qbar repo root
const REPO_ROOT = join(import.meta.dir, "..");

async function runCmd(
  cmd: string,
  args: string[],
  cwd: string,
): Promise<{ ok: boolean; output: string }> {
  try {
    const proc = Bun.spawn([cmd, ...args], {
      cwd,
      stdout: "pipe",
      stderr: "pipe",
    });

    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    const code = await proc.exited;

    return { ok: code === 0, output: stdout + stderr };
  } catch (error) {
    return { ok: false, output: String(error) };
  }
}

export async function main() {
  console.clear();

  p.intro(colorize("qbar update", catppuccin.mauve));

  // Check if we're in a git repo
  const gitCheck = await runCmd("git", ["rev-parse", "--git-dir"], REPO_ROOT);
  if (!gitCheck.ok) {
    p.log.error(
      colorize("Not a git repository. Cannot update.", semantic.danger),
    );
    p.outro(colorize("Update failed", semantic.danger));
    return;
  }

  // Get current version
  const currentCommit = await runCmd(
    "git",
    ["rev-parse", "--short", "HEAD"],
    REPO_ROOT,
  );
  const currentBranch = await runCmd(
    "git",
    ["branch", "--show-current"],
    REPO_ROOT,
  );

  p.log.info(
    colorize(`Branch: ${currentBranch.output.trim()}`, semantic.subtitle),
  );
  p.log.info(
    colorize(`Current: ${currentCommit.output.trim()}`, semantic.subtitle),
  );

  // Fetch latest
  const spinner = p.spinner();
  spinner.start("Fetching latest changes...");

  const fetch = await runCmd("git", ["fetch", "origin"], REPO_ROOT);
  if (!fetch.ok) {
    spinner.stop(colorize("Failed to fetch", semantic.danger));
    p.log.error(fetch.output);
    return;
  }

  // Check if there are updates
  const status = await runCmd("git", ["status", "-uno"], REPO_ROOT);
  const behind = status.output.includes("behind");

  if (!behind) {
    spinner.stop(colorize("Already up to date!", semantic.good));
    p.outro(colorize("No updates available", semantic.subtitle));
    return;
  }

  spinner.stop(colorize("Updates available", semantic.good));

  // Show what's coming
  const log = await runCmd(
    "git",
    ["log", "--oneline", "HEAD..origin/master", "-10"],
    REPO_ROOT,
  );
  if (log.ok && log.output.trim()) {
    p.note(
      log.output
        .trim()
        .split("\n")
        .map((l) => colorize(l, semantic.subtitle))
        .join("\n"),
      colorize("New commits", semantic.title),
    );
  }

  const proceed = await p.confirm({
    message: "Apply update?",
    initialValue: true,
  });

  if (p.isCancel(proceed) || !proceed) {
    p.outro(colorize("Update cancelled", semantic.muted));
    return;
  }

  // Pull changes
  spinner.start("Pulling changes...");
  const pull = await runCmd("git", ["pull", "--ff-only"], REPO_ROOT);

  if (!pull.ok) {
    spinner.stop(colorize("Pull failed", semantic.danger));
    p.log.error(pull.output);
    p.log.warn(
      colorize("Try: git stash && git pull && git stash pop", semantic.warning),
    );
    return;
  }

  spinner.stop(colorize("Code updated", semantic.good));

  // Install dependencies
  spinner.start("Installing dependencies...");
  const install = await runCmd("bun", ["install"], REPO_ROOT);

  if (!install.ok) {
    spinner.stop(colorize("Install failed", semantic.danger));
    p.log.error(install.output);
    return;
  }

  spinner.stop(colorize("Dependencies updated", semantic.good));

  // Get new version
  const newCommit = await runCmd(
    "git",
    ["rev-parse", "--short", "HEAD"],
    REPO_ROOT,
  );

  p.log.success(
    colorize(
      `Updated: ${currentCommit.output.trim()} → ${newCommit.output.trim()}`,
      semantic.good,
    ),
  );

  // Reload waybar
  try {
    Bun.spawn(["pkill", "-USR2", "waybar"]);
  } catch {}

  p.outro(colorize("Update complete!", semantic.good));
}

// Only auto-run when executed directly
if (import.meta.main) {
  main().catch((e) => {
    console.error("Update failed:", e);
    process.exit(1);
  });
}

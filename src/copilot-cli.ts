import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export const COPILOT_MISSING_ERROR =
  "GitHub Copilot CLI not installed. Install `copilot` and log in.";

export function getCopilotCandidatePaths(home = homedir()): string[] {
  return [
    join(home, ".local", "bin", "copilot"),
    join(home, ".cache", ".bun", "bin", "copilot"),
    join(home, ".bun", "bin", "copilot"),
  ];
}

export function findCopilotBin(
  options: {
    home?: string;
    which?: (cmd: string) => string | null;
    exists?: (path: string) => boolean;
  } = {},
) {
  const which =
    options.which ?? (typeof Bun.which === "function" ? Bun.which : undefined);
  const foundFromPath = which?.("copilot");
  if (foundFromPath) return foundFromPath;

  const exists = options.exists ?? existsSync;
  for (const candidate of getCopilotCandidatePaths(options.home)) {
    if (exists(candidate)) return candidate;
  }

  return null;
}

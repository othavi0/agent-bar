import { describe, expect, it } from "bun:test";
import {
  AMP_INSTALL_COMMAND,
  AMP_MISSING_ERROR,
  findAmpBin,
  getAmpCandidatePaths,
} from "../src/amp-cli";

describe("amp-cli helpers", () => {
  it("uses the official Amp install command", () => {
    expect(AMP_INSTALL_COMMAND).toBe(
      "curl -fsSL https://ampcode.com/install.sh | bash",
    );
  });

  it("returns the current missing-cli hint", () => {
    expect(AMP_MISSING_ERROR).toContain("Right-click");
  });

  it("checks common install paths under HOME", () => {
    expect(getAmpCandidatePaths("/tmp/agent-bar-home")).toEqual([
      "/tmp/agent-bar-home/.local/bin/amp",
      "/tmp/agent-bar-home/.amp/bin/amp",
      "/tmp/agent-bar-home/.cache/.bun/bin/amp",
      "/tmp/agent-bar-home/.bun/bin/amp",
    ]);
  });

  it("prefers amp from PATH when available", () => {
    const found = findAmpBin({
      home: "/tmp/agent-bar-home",
      which: () => "/usr/local/bin/amp",
      exists: () => false,
    });

    expect(found).toBe("/usr/local/bin/amp");
  });

  it("falls back to the known install locations", () => {
    const home = "/tmp/agent-bar-home";
    const found = findAmpBin({
      home,
      which: () => null,
      exists: (path) => path === "/tmp/agent-bar-home/.local/bin/amp",
    });

    expect(found).toBe("/tmp/agent-bar-home/.local/bin/amp");
  });

  it("returns null when amp is unavailable", () => {
    const found = findAmpBin({
      home: "/tmp/agent-bar-home",
      which: () => null,
      exists: () => false,
    });

    expect(found).toBeNull();
  });
});

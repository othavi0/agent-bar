import { describe, expect, it } from "bun:test";
import { formatForTerminal } from "../src/formatters/terminal";
import { formatForWaybar, formatProviderForWaybar } from "../src/formatters/waybar";
import type { AllQuotas, ProviderQuota } from "../src/providers/types";
import { ONE_DARK, ANSI, BOX } from "../src/theme";

function mockClaudeQuota(remaining: number): ProviderQuota {
  return {
    provider: "claude",
    available: true,
    primary: {
      remaining,
      limit: 100,
      used: 100 - remaining,
      windowMinutes: 300,
      resetsAt: new Date(Date.now() + 3600000).toISOString(),
    },
  };
}

function mockCodexQuota(remaining: number): ProviderQuota {
  return {
    provider: "codex",
    available: true,
    primary: {
      remaining,
      limit: 100,
      used: 100 - remaining,
      windowMinutes: 300,
      resetsAt: new Date(Date.now() + 3600000).toISOString(),
    },
  };
}

function mockAmpQuota(): ProviderQuota {
  return {
    provider: "amp",
    available: true,
    models: {
      "Free Tier": {
        remaining: 75,
        limit: 100,
        used: 25,
        windowMinutes: 1440,
        resetsAt: new Date(Date.now() + 7200000).toISOString(),
      },
    },
  };
}

function mockAllQuotas(providers: ProviderQuota[]): AllQuotas {
  return {
    providers,
    fetchedAt: new Date().toISOString(),
  };
}

describe("formatForTerminal", () => {
  it("returns 'No providers connected' when empty", () => {
    const result = formatForTerminal({ providers: [], fetchedAt: new Date().toISOString() });
    expect(result).toContain("No providers connected");
  });

  it("renders Claude section with box-drawing chars", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForTerminal(quotas);

    expect(result).toContain("Claude");
    expect(result).toContain(BOX.tl);
    expect(result).toContain(BOX.bl);
    expect(result).toContain("█");
  });

  it("renders Codex section", () => {
    const quotas = mockAllQuotas([mockCodexQuota(45)]);
    const result = formatForTerminal(quotas);

    expect(result).toContain("Codex");
    expect(result).toContain(BOX.tl);
  });

  it("renders Amp section", () => {
    const quotas = mockAllQuotas([mockAmpQuota()]);
    const result = formatForTerminal(quotas);

    expect(result).toContain("Amp");
    expect(result).toContain("Free Tier");
  });

  it("renders multiple providers separated by double newline", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80), mockCodexQuota(45)]);
    const result = formatForTerminal(quotas);

    expect(result).toContain("Claude");
    expect(result).toContain("Codex");
    expect(result).toContain("\n\n");
  });

  it("shows error message for provider with error", () => {
    const quota: ProviderQuota = {
      provider: "claude",
      available: true,
      error: "Token expired",
    };
    const result = formatForTerminal(mockAllQuotas([quota]));

    expect(result).toContain("Token expired");
  });

  it("skips unavailable providers without errors", () => {
    const quota: ProviderQuota = {
      provider: "claude",
      available: false,
    };
    const result = formatForTerminal(mockAllQuotas([quota]));

    expect(result).toContain("No providers connected");
  });

  it("uses ANSI color codes in output", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForTerminal(quotas);

    // Should contain ANSI escape sequences
    expect(result).toContain("\x1b[");
    expect(result).toContain(ANSI.reset);
  });
});

describe("formatForWaybar", () => {
  it("returns WaybarOutput shape", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result).toHaveProperty("text");
    expect(result).toHaveProperty("tooltip");
    expect(result).toHaveProperty("class");
  });

  it("uses Pango markup in tooltip", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result.tooltip).toContain("<span");
    expect(result.tooltip).toContain("foreground=");
    expect(result.tooltip).toContain("</span>");
  });

  it("uses hex colors (not ANSI) in tooltip", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    // Should contain hex colors like #d19a66
    expect(result.tooltip).toMatch(/#[0-9a-f]{6}/i);
    // Should NOT contain ANSI escape sequences
    expect(result.tooltip).not.toContain("\x1b[");
  });

  it("includes box-drawing chars in tooltip", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result.tooltip).toContain(BOX.tl);
    expect(result.tooltip).toContain(BOX.bl);
    expect(result.tooltip).toContain(BOX.v);
  });

  it("sets class with provider status", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForWaybar(quotas);

    expect(result.class).toContain("qbar");
    expect(result.class).toContain("claude-ok");
  });

  it("sets critical class for very low quota", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(5)]);
    const result = formatForWaybar(quotas);

    expect(result.class).toContain("claude-critical");
  });

  it("sets warn class for low quota", () => {
    const quotas = mockAllQuotas([mockClaudeQuota(20)]);
    const result = formatForWaybar(quotas);

    expect(result.class).toContain("claude-warn");
  });

  it("shows 'No Providers' when empty", () => {
    const quotas = mockAllQuotas([]);
    const result = formatForWaybar(quotas);

    expect(result.text).toContain("No Providers");
  });
});

describe("formatProviderForWaybar", () => {
  it("returns disconnected state for unavailable provider", () => {
    const quota: ProviderQuota = {
      provider: "claude",
      available: false,
      error: "No credentials",
    };
    const result = formatProviderForWaybar(quota);

    expect(result.class).toContain("disconnected");
    expect(result.text).toContain(ONE_DARK.red);
  });

  it("returns percentage for available provider", () => {
    const result = formatProviderForWaybar(mockClaudeQuota(80));

    expect(result.class).toContain("qbar-claude");
    expect(result.tooltip).toContain("Claude");
  });
});

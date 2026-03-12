import { describe, expect, it } from "bun:test";
import { ONE_DARK, ANSI, PROVIDER_HEX, PROVIDER_ANSI, BOX } from "../src/theme";

describe("ONE_DARK hex palette", () => {
  it("has all core colors as valid hex strings", () => {
    const hexPattern = /^#[0-9a-f]{6}$/i;
    for (const [key, val] of Object.entries(ONE_DARK)) {
      expect(val).toMatch(hexPattern);
    }
  });

  it("contains expected color keys", () => {
    const expected = [
      "background", "surface", "overlay", "selection", "selectionAlt",
      "text", "textBright", "comment", "muted",
      "red", "green", "yellow", "blue", "magenta", "cyan", "orange",
      "brightBlue", "brightMagenta",
      "borderSoft", "borderStrong",
    ];
    for (const key of expected) {
      expect(ONE_DARK).toHaveProperty(key);
    }
  });
});

describe("ANSI escape codes", () => {
  it("generates true-color escape sequences", () => {
    // Green = #98c379 → rgb(152,195,121)
    expect(ANSI.green).toBe("\x1b[38;2;152;195;121m");
  });

  it("has reset and formatting codes", () => {
    expect(ANSI.reset).toBe("\x1b[0m");
    expect(ANSI.bold).toBe("\x1b[1m");
    expect(ANSI.dim).toBe("\x1b[2m");
  });

  it("maps all ONE_DARK accent colors", () => {
    const colorKeys = [
      "green", "yellow", "orange", "red", "blue", "cyan", "magenta",
      "text", "textBright", "comment", "muted",
      "borderSoft", "borderStrong", "brightBlue", "brightMagenta",
    ];
    for (const key of colorKeys) {
      const val = (ANSI as Record<string, string>)[key];
      expect(val).toContain("\x1b[38;2;");
    }
  });
});

describe("PROVIDER_HEX", () => {
  it("maps claude to orange", () => {
    expect(PROVIDER_HEX.claude).toBe(ONE_DARK.orange);
  });

  it("maps codex to green", () => {
    expect(PROVIDER_HEX.codex).toBe(ONE_DARK.green);
  });

  it("maps amp to magenta", () => {
    expect(PROVIDER_HEX.amp).toBe(ONE_DARK.magenta);
  });
});

describe("PROVIDER_ANSI", () => {
  it("maps claude to orange ANSI", () => {
    expect(PROVIDER_ANSI.claude).toBe(ANSI.orange);
  });

  it("maps codex to green ANSI", () => {
    expect(PROVIDER_ANSI.codex).toBe(ANSI.green);
  });

  it("maps amp to magenta ANSI", () => {
    expect(PROVIDER_ANSI.amp).toBe(ANSI.magenta);
  });
});

describe("BOX drawing characters", () => {
  it("exports all expected characters", () => {
    expect(BOX.tl).toBe("┏");
    expect(BOX.bl).toBe("┗");
    expect(BOX.lt).toBe("┣");
    expect(BOX.h).toBe("━");
    expect(BOX.v).toBe("┃");
    expect(BOX.dot).toBe("●");
    expect(BOX.dotO).toBe("○");
    expect(BOX.diamond).toBe("◆");
  });
});

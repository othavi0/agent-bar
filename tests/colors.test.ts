import { describe, expect, it } from "bun:test";
import { oneDark, semantic, getQuotaColor, colorize, bold, dim } from "../src/tui/colors";
import { ANSI } from "../src/theme";

describe("oneDark re-exports", () => {
  it("mirrors ANSI values", () => {
    expect(oneDark.green).toBe(ANSI.green);
    expect(oneDark.orange).toBe(ANSI.orange);
    expect(oneDark.cyan).toBe(ANSI.cyan);
    expect(oneDark.magenta).toBe(ANSI.magenta);
    expect(oneDark.textBright).toBe(ANSI.textBright);
    expect(oneDark.reset).toBe(ANSI.reset);
    expect(oneDark.bold).toBe(ANSI.bold);
    expect(oneDark.dim).toBe(ANSI.dim);
  });
});

describe("semantic color mapping", () => {
  it("maps good to green", () => {
    expect(semantic.good).toBe(oneDark.green);
  });

  it("maps warning to yellow", () => {
    expect(semantic.warning).toBe(oneDark.yellow);
  });

  it("maps danger to orange", () => {
    expect(semantic.danger).toBe(oneDark.orange);
  });

  it("maps critical to red", () => {
    expect(semantic.critical).toBe(oneDark.red);
  });

  it("maps muted to comment", () => {
    expect(semantic.muted).toBe(oneDark.comment);
  });

  it("maps accent to blue", () => {
    expect(semantic.accent).toBe(oneDark.blue);
  });

  it("maps title to textBright", () => {
    expect(semantic.title).toBe(oneDark.textBright);
  });

  it("maps highlight to cyan", () => {
    expect(semantic.highlight).toBe(oneDark.cyan);
  });
});

describe("getQuotaColor", () => {
  it("returns muted for null", () => {
    expect(getQuotaColor(null)).toBe(semantic.muted);
  });

  it("returns green for >= 60%", () => {
    expect(getQuotaColor(60)).toBe(semantic.good);
    expect(getQuotaColor(100)).toBe(semantic.good);
    expect(getQuotaColor(75)).toBe(semantic.good);
  });

  it("returns yellow for >= 30% and < 60%", () => {
    expect(getQuotaColor(30)).toBe(semantic.warning);
    expect(getQuotaColor(45)).toBe(semantic.warning);
    expect(getQuotaColor(59)).toBe(semantic.warning);
  });

  it("returns orange for >= 10% and < 30%", () => {
    expect(getQuotaColor(10)).toBe(semantic.danger);
    expect(getQuotaColor(20)).toBe(semantic.danger);
    expect(getQuotaColor(29)).toBe(semantic.danger);
  });

  it("returns red for < 10%", () => {
    expect(getQuotaColor(0)).toBe(semantic.critical);
    expect(getQuotaColor(5)).toBe(semantic.critical);
    expect(getQuotaColor(9)).toBe(semantic.critical);
  });
});

describe("colorize", () => {
  it("wraps text with color and reset", () => {
    const result = colorize("hello", oneDark.green);
    expect(result).toBe(`${oneDark.green}hello${oneDark.reset}`);
  });

  it("adds bold prefix when isBold is true", () => {
    const result = colorize("hello", oneDark.green, true);
    expect(result).toBe(`${oneDark.bold}${oneDark.green}hello${oneDark.reset}`);
  });

  it("does not add bold prefix when isBold is false", () => {
    const result = colorize("hello", oneDark.green, false);
    expect(result).not.toContain(oneDark.bold);
  });
});

describe("bold", () => {
  it("wraps text with bold and reset", () => {
    const result = bold("hello");
    expect(result).toBe(`${oneDark.bold}hello${oneDark.reset}`);
  });
});

describe("dim", () => {
  it("wraps text with dim and reset", () => {
    const result = dim("hello");
    expect(result).toBe(`${oneDark.dim}hello${oneDark.reset}`);
  });
});

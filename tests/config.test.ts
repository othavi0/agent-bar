import { describe, expect, it } from "bun:test";
import { CONFIG, getColorForPercent } from "../src/config";
import { ONE_DARK } from "../src/theme";

describe("CONFIG.colors", () => {
  it("uses ONE_DARK hex values", () => {
    expect(CONFIG.colors.green).toBe(ONE_DARK.green);
    expect(CONFIG.colors.yellow).toBe(ONE_DARK.yellow);
    expect(CONFIG.colors.orange).toBe(ONE_DARK.orange);
    expect(CONFIG.colors.red).toBe(ONE_DARK.red);
    expect(CONFIG.colors.muted).toBe(ONE_DARK.comment);
    expect(CONFIG.colors.text).toBe(ONE_DARK.text);
  });
});

describe("CONFIG.thresholds", () => {
  it("has correct threshold values", () => {
    expect(CONFIG.thresholds.green).toBe(60);
    expect(CONFIG.thresholds.yellow).toBe(30);
    expect(CONFIG.thresholds.orange).toBe(10);
  });
});

describe("getColorForPercent", () => {
  it("returns text color for null", () => {
    expect(getColorForPercent(null)).toBe(CONFIG.colors.text);
  });

  it("returns green for >= 60%", () => {
    expect(getColorForPercent(60)).toBe(CONFIG.colors.green);
    expect(getColorForPercent(100)).toBe(CONFIG.colors.green);
  });

  it("returns yellow for >= 30% and < 60%", () => {
    expect(getColorForPercent(30)).toBe(CONFIG.colors.yellow);
    expect(getColorForPercent(59)).toBe(CONFIG.colors.yellow);
  });

  it("returns orange for >= 10% and < 30%", () => {
    expect(getColorForPercent(10)).toBe(CONFIG.colors.orange);
    expect(getColorForPercent(29)).toBe(CONFIG.colors.orange);
  });

  it("returns red for < 10%", () => {
    expect(getColorForPercent(0)).toBe(CONFIG.colors.red);
    expect(getColorForPercent(9)).toBe(CONFIG.colors.red);
  });

  it("handles edge cases", () => {
    expect(getColorForPercent(-1)).toBe(CONFIG.colors.red);
    expect(getColorForPercent(200)).toBe(CONFIG.colors.green);
  });
});

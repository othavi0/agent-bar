import type { ColorToken } from '../segments';
import type { DisplayMode } from '../shared';

export interface BuildOptions {
  mode: DisplayMode;
  /**
   * Full title string rendered in the header.
   * Terminal: "Claude", Waybar: "Claude · Pro" (when plan is known).
   */
  headerTitle: string;
  /**
   * Total visual width of the header line (in chars).
   * The builder derives the dash fill as `max(1, headerWidth - headerTitle.length)`.
   * Terminal: 56 (fill=50 for "Claude"), Waybar: TOOLTIP_BORDER - 4 = 52.
   */
  headerWidth: number;
  /**
   * Accent color used for the `◆ Label` part of section labels.
   * Terminal: 'magenta', Waybar: 'orange' (provider color), TUI: 'blue'.
   */
  labelColor: ColorToken;
  /**
   * Footer options. When undefined the footer is a simple 55-dash line.
   * When provided and fetchedAt is set, the footer includes a cached stamp.
   */
  footer?: { fetchedAt?: string };
  /**
   * Plan label string for providers that emit a separate "Plan: X" row
   * (e.g. Codex in terminal/TUI). When undefined the row is omitted.
   * Waybar embeds the plan into headerTitle instead and leaves this unset.
   */
  planLabel?: string;
  /**
   * Controls how Amp renders the Free Tier section.
   *
   * - 'sublines'  — Terminal: bar + tree-connector sub-lines (├─/└─) for
   *                 dollar info and ETA. No inline ETA on the bar line.
   * - 'inline'    — Waybar: bar + inline ETA on the bar line + a dot (○) line
   *                 for dollar info. No tree connectors.
   * - 'generic'   — TUI: simplified generic model loop (no Free Tier special
   *                 handling; iterates all models with name+bar+pct).
   *
   * When undefined, defaults to 'inline'.
   */
  ampFreeTierLayout?: 'sublines' | 'inline' | 'generic';
}

import { describe, expect, it } from "bun:test";
import { exportWaybarModules } from "../src/waybar-contract";

describe("exportWaybarModules", () => {
  it("wires left and right click handlers through the terminal helper", () => {
    const result = exportWaybarModules(
      {
        appBin: "$HOME/.local/bin/agent-bar-omarchy",
        terminalScript: "$HOME/.config/waybar/scripts/agent-bar-omarchy-open-terminal",
      },
      ["claude", "codex", "amp"],
    );

    expect(result.modules["custom/agent-bar-omarchy-claude"]["on-click"]).toBe(
      "$HOME/.config/waybar/scripts/agent-bar-omarchy-open-terminal $HOME/.local/bin/agent-bar-omarchy menu",
    );
    expect(result.modules["custom/agent-bar-omarchy-codex"]["exec-on-event"]).toBe(true);
    expect(result.modules["custom/agent-bar-omarchy-amp"]["on-click-right"]).toBe(
      "$HOME/.config/waybar/scripts/agent-bar-omarchy-open-terminal $HOME/.local/bin/agent-bar-omarchy action-right amp",
    );
  });
});

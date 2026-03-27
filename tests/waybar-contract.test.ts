import { describe, expect, it } from "bun:test";
import { exportWaybarModules } from "../src/waybar-contract";

describe("exportWaybarModules", () => {
  it("wires left and right click handlers through the terminal helper", () => {
    const result = exportWaybarModules(
      {
        qbarBin: "$HOME/.local/bin/qbar",
        terminalScript: "$HOME/.config/waybar/scripts/qbar-open-terminal",
      },
      ["claude", "codex", "amp"],
    );

    expect(result.modules["custom/qbar-claude"]["on-click"]).toBe(
      "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
    );
    expect(result.modules["custom/qbar-codex"]["exec-on-event"]).toBe(true);
    expect(result.modules["custom/qbar-amp"]["on-click-right"]).toBe(
      "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right amp",
    );
  });
});

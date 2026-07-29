// Test-only palette + screenshot inventory fixtures.
.pragma library

// Screenshot inventory required by TEST/CP2 (exact basenames).
function requiredScreenshotNames() {
  return [
    "ready-light.png",
    "ready-dark.png",
    "loading-dark.png",
    "refreshing-with-data-dark.png",
    "stale-dark.png",
    "cli-missing-dark.png",
    "unauthenticated-dark.png",
    "rate-limited-dark.png",
    "network-error-dark.png",
    "provider-error-dark.png",
    "settings-clean-dark.png",
    "settings-dirty-dark.png",
    "settings-invalid-dark.png",
    "maintenance-update-dark.png",
    "uninstall-confirmation-dark.png"
  ]
}

function themePalette(mode) {
  if (mode === "light") {
    return {
      mode: "light",
      background: "#f4f4f5",
      foreground: "#18181b",
      muted: "#52525b",
      border: "#d4d4d8",
      urgent: "#b91c1c"
    }
  }
  return {
    mode: "dark",
    background: "#18181b",
    foreground: "#e4e4e7",
    muted: "#a1a1aa",
    border: "#3f3f46",
    urgent: "#f87171"
  }
}

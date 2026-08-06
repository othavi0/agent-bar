// Maintenance state: handoff/lane guard, login, update, uninstall UI.
.pragma library
.import "CoreService.js" as Kernel

// ---------------------------------------------------------------------------
// Maintenance state
// ---------------------------------------------------------------------------

function maintenanceIdle() {
  return { phase: "idle", blocked: false }
}

function maintenanceBeginHandoff(state) {
  return { phase: "handoff", blocked: true }
}

function maintenanceCanStartWrite(maint) {
  return !maint || !maint.blocked
}

// Drain rule: handoff waits while status or settingsWrite busy.
function maintenanceCanDetach(maint, statusBusy, settingsWriteBusy) {
  if (!maint || maint.phase !== "handoff")
    return false
  return !statusBusy && !settingsWriteBusy
}

// ---------------------------------------------------------------------------
// Login / maintenance UI (Task 13 — UX-040..048)
// ---------------------------------------------------------------------------

function loginDetachedArgv(pluginRoot, providerId) {
  if (!pluginRoot || !String(pluginRoot).length)
    return null
  if (!Kernel.isClosedProvider(providerId))
    return null
  return [
    String(pluginRoot) + "/scripts/agent-bar-open-terminal",
    "login",
    String(providerId)
  ]
}

// Exact xdg-terminal-exec argv the Bash helper must exec (ARCH login flow).
function terminalHelperXdgArgv(pluginRoot, providerId) {
  if (!pluginRoot || !Kernel.isClosedProvider(providerId))
    return null
  return [
    "xdg-terminal-exec",
    "--app-id=org.omarchy.terminal",
    "--title=Agent Bar Login",
    "--",
    String(pluginRoot) + "/bin/agent-bar",
    "login",
    String(providerId)
  ]
}

function updateCheckArgv(helperPath) {
  return [String(helperPath), "update", "check"]
}

function updateApplyArgv(helperPath) {
  return [String(helperPath), "update", "apply"]
}

function uninstallArgv(helperPath, purge) {
  if (purge)
    return [String(helperPath), "uninstall", "purge"]
  return [String(helperPath), "uninstall"]
}

// Non-TTY uninstall confirmation document (CLI contract).
function uninstallConfirmation(purge) {
  return {
    schemaVersion: 1,
    operation: "uninstall",
    confirmed: true,
    purgeSettingsAndBackups: !!purge
  }
}

function maintenanceUiIdle(installedVersion) {
  return {
    phase: "idle",
    installedVersion: installedVersion ? String(installedVersion) : "",
    targetVersion: "",
    releaseNotesUrl: "",
    purgeSettings: false,
    uninstallArmed: false,
    message: "",
    updateConfirmOpen: false,
    uninstallConfirmOpen: false
  }
}

function maintenanceUiChecking(ui) {
  var next = cloneMaintenanceUi(ui)
  next.phase = "checking"
  next.message = "Checking for updates\u2026"
  next.updateConfirmOpen = false
  return next
}

function cloneMaintenanceUi(ui) {
  return {
    phase: ui && ui.phase ? ui.phase : "idle",
    installedVersion: ui && ui.installedVersion ? String(ui.installedVersion) : "",
    targetVersion: ui && ui.targetVersion ? String(ui.targetVersion) : "",
    releaseNotesUrl: ui && ui.releaseNotesUrl ? String(ui.releaseNotesUrl) : "",
    purgeSettings: !!(ui && ui.purgeSettings),
    uninstallArmed: !!(ui && ui.uninstallArmed),
    message: ui && ui.message ? String(ui.message) : "",
    updateConfirmOpen: !!(ui && ui.updateConfirmOpen),
    uninstallConfirmOpen: !!(ui && ui.uninstallConfirmOpen)
  }
}

// Parse `update check` stdout: the BUNDLE-021 document
// { schemaVersion, checkedAt, current, available, latestCompatible }.
// A successful check always writes exactly that JSON, so exit 0 with
// anything else is a failed check, not an implied answer. The fixtures in
// tests/fixtures/update-check/ are pinned byte-exactly to the Rust
// serializer by tests/update_check_parity.rs.
function maintenanceUiFromCheck(ui, stdout, exitCode, fallbackVersion) {
  var next = cloneMaintenanceUi(ui)
  next.updateConfirmOpen = false
  if (exitCode === 0) {
    try {
      var doc = JSON.parse(String(stdout || ""))
      if (doc && doc.schemaVersion === 1) {
        var current = doc.current && doc.current.version ? String(doc.current.version) : ""
        next.installedVersion = current.length
            ? current
            : String(next.installedVersion || fallbackVersion || "")
        if (doc.reinstallRequired === true) {
          next.phase = "reinstall_required"
          next.targetVersion = ""
          next.releaseNotesUrl = ""
          next.message = "Installed without git. Run: omarchy plugin remove agent-bar.usage, "
              + "then omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git"
          return next
        }
        var latest = doc.latestCompatible
        if (doc.available === true && latest && latest.version) {
          next.phase = "update_available"
          next.targetVersion = String(latest.version)
          next.releaseNotesUrl = latest.releaseNotesUrl ? String(latest.releaseNotesUrl) : ""
          next.message = "Update to " + next.targetVersion + " is available."
          return next
        }
        // latestCompatible may be null (nothing fits this target/contract)
        // or describe the installed version; either way there is nothing
        // to offer.
        if (doc.available === false) {
          next.phase = "up_to_date"
          next.targetVersion = ""
          next.releaseNotesUrl = ""
          next.message = "Agent Bar is up to date."
          return next
        }
      }
    } catch (e) {
      // unusable stdout falls through to the single failure exit
    }
  }
  next.phase = "error"
  next.message = "Update check failed."
  return next
}

function maintenanceUiOpenUpdateConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  if (next.phase !== "update_available" || !next.targetVersion.length)
    return next
  next.updateConfirmOpen = true
  return next
}

function maintenanceUiCloseUpdateConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  next.updateConfirmOpen = false
  return next
}

function updateConfirmMessage(ui) {
  var current = ui && ui.installedVersion ? String(ui.installedVersion) : "current"
  var target = ui && ui.targetVersion ? String(ui.targetVersion) : "new"
  return "Updates " + current + " \u2192 " + target
      + ". Settings stay. Fast-forwards to the latest release; a failed validation rolls back."
}

function maintenanceUiOpenUninstallConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  next.uninstallConfirmOpen = true
  next.uninstallArmed = false
  next.purgeSettings = false
  return next
}

function maintenanceUiCloseUninstallConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  next.uninstallConfirmOpen = false
  next.uninstallArmed = false
  return next
}

function maintenanceUiSetPurge(ui, purge) {
  var next = cloneMaintenanceUi(ui)
  next.purgeSettings = !!purge
  // Changing purge resets the second destructive arm (UX-047 safety).
  next.uninstallArmed = false
  return next
}

// First destructive click arms; second confirms (UX-047).
function maintenanceUiArmOrConfirmUninstall(ui) {
  var next = cloneMaintenanceUi(ui)
  if (!next.uninstallConfirmOpen)
    return { ui: next, confirmed: false }
  if (!next.uninstallArmed) {
    next.uninstallArmed = true
    return { ui: next, confirmed: false }
  }
  return { ui: next, confirmed: true }
}

function maintenanceUiApplying(ui) {
  var next = cloneMaintenanceUi(ui)
  next.phase = "applying"
  next.updateConfirmOpen = false
  next.message = "Applying update\u2026"
  return next
}

function maintenanceUiUninstalling(ui) {
  var next = cloneMaintenanceUi(ui)
  next.phase = "uninstalling"
  next.uninstallConfirmOpen = false
  next.message = "Uninstalling\u2026"
  return next
}

function maintenanceIntention(kind, ui) {
  if (kind === "update_apply") {
    return {
      kind: "update_apply",
      version: ui && ui.targetVersion ? String(ui.targetVersion) : "",
      payload: null
    }
  }
  if (kind === "uninstall") {
    return {
      kind: "uninstall",
      purge: !!(ui && ui.purgeSettings),
      payload: uninstallConfirmation(!!(ui && ui.purgeSettings))
    }
  }
  return null
}

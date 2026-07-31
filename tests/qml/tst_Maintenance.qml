import QtQuick
import QtTest
import "../../assets/omarchy/CoreMaintenance.js" as Core

TestCase {
  id: testCase
  name: "AgentBarMaintenance"
  when: windowShown

  property string repoRoot: {
    var u = Qt.resolvedUrl(".")
    var path = String(u).replace("file://", "")
    if (path.endsWith("/"))
      path = path.slice(0, -1)
    var parts = path.split("/")
    parts.pop(); parts.pop()
    return parts.join("/")
  }

  function read(rel) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + repoRoot + "/" + rel, false)
    xhr.send()
    return String(xhr.responseText || "")
  }

  // ---- Login argv ----

  function test_login_detached_argv() {
    var argv = Core.loginDetachedArgv("/home/u/.config/omarchy/plugins/agent-bar.usage", "claude")
    compare(argv.length, 3)
    compare(argv[0], "/home/u/.config/omarchy/plugins/agent-bar.usage/scripts/agent-bar-open-terminal")
    compare(argv[1], "login")
    compare(argv[2], "claude")
    compare(Core.loginDetachedArgv("/x", "nope"), null)
    compare(Core.loginDetachedArgv("", "claude"), null)
  }

  function test_terminal_helper_xdg_argv_exact() {
    var argv = Core.terminalHelperXdgArgv("/plugin", "amp")
    compare(argv[0], "xdg-terminal-exec")
    compare(argv[1], "--app-id=org.omarchy.terminal")
    compare(argv[2], "--title=Agent Bar Login")
    compare(argv[3], "--")
    compare(argv[4], "/plugin/bin/agent-bar")
    compare(argv[5], "login")
    compare(argv[6], "amp")
  }

  // ---- Update / uninstall argv + confirmation ----

  function test_update_and_uninstall_argv() {
    var check = Core.updateCheckArgv("/bin/agent-bar")
    compare(check.join(" "), "/bin/agent-bar update check")
    var apply = Core.updateApplyArgv("/bin/agent-bar", "10.1.0")
    compare(apply.join(" "), "/bin/agent-bar update apply 10.1.0")
    compare(Core.updateApplyArgv("/bin/agent-bar", ""), null)
    compare(Core.uninstallArgv("/bin/agent-bar", false).join(" "), "/bin/agent-bar uninstall")
    compare(Core.uninstallArgv("/bin/agent-bar", true).join(" "), "/bin/agent-bar uninstall purge")
  }

  function test_uninstall_confirmation_json() {
    var keep = Core.uninstallConfirmation(false)
    compare(keep.schemaVersion, 1)
    compare(keep.operation, "uninstall")
    compare(keep.confirmed, true)
    compare(keep.purgeSettingsAndBackups, false)
    var purge = Core.uninstallConfirmation(true)
    compare(purge.purgeSettingsAndBackups, true)
  }

  function test_update_check_parse_available() {
    var ui = Core.maintenanceUiIdle("10.0.0")
    var json = JSON.stringify({
      updateAvailable: true,
      currentVersion: "10.0.0",
      targetVersion: "10.1.0",
      releaseNotesUrl: "https://example.com/notes"
    })
    ui = Core.maintenanceUiFromCheck(ui, json, 0, "10.0.0")
    compare(ui.phase, "update_available")
    compare(ui.targetVersion, "10.1.0")
    compare(ui.releaseNotesUrl, "https://example.com/notes")
    verify(ui.message.indexOf("10.1.0") >= 0)
  }

  function test_update_check_up_to_date() {
    var ui = Core.maintenanceUiIdle("10.0.0")
    ui = Core.maintenanceUiFromCheck(ui, JSON.stringify({ updateAvailable: false, currentVersion: "10.0.0" }), 0, "10.0.0")
    compare(ui.phase, "up_to_date")
    ui = Core.maintenanceUiFromCheck(Core.maintenanceUiIdle("10.0.0"), "Agent Bar is up to date.\n", 0, "10.0.0")
    compare(ui.phase, "up_to_date")
  }

  function test_update_confirm_message_names_versions() {
    var ui = Core.maintenanceUiIdle("10.0.0")
    ui.targetVersion = "10.2.0"
    var msg = Core.updateConfirmMessage(ui)
    verify(msg.indexOf("10.0.0") >= 0)
    verify(msg.indexOf("10.2.0") >= 0)
    verify(msg.toLowerCase().indexOf("settings") >= 0)
    verify(msg.toLowerCase().indexOf("roll back") >= 0 || msg.toLowerCase().indexOf("rollback") >= 0)
    // §5.6 shortened it; the old sentence must not come back.
    verify(msg.indexOf("This replaces the plugin bundle") < 0)
  }

  function test_update_check_failure_has_one_string() {
    var src = read("assets/omarchy/CoreMaintenance.js")
    verify(src.indexOf("Update check returned an unusable response.") < 0)
    var first = src.indexOf("Update check failed.")
    verify(first >= 0)
    verify(src.indexOf("Update check failed.", first + 1) > first,
           "both failure branches must use the one string")
  }

  function test_uninstall_double_confirm() {
    var ui = Core.maintenanceUiOpenUninstallConfirm(Core.maintenanceUiIdle("10.0.0"))
    compare(ui.uninstallConfirmOpen, true)
    compare(ui.purgeSettings, false)
    compare(ui.uninstallArmed, false)
    var r1 = Core.maintenanceUiArmOrConfirmUninstall(ui)
    compare(r1.confirmed, false)
    compare(r1.ui.uninstallArmed, true)
    var r2 = Core.maintenanceUiArmOrConfirmUninstall(r1.ui)
    compare(r2.confirmed, true)
  }

  function test_purge_toggle_resets_arm() {
    var ui = Core.maintenanceUiOpenUninstallConfirm(Core.maintenanceUiIdle("10.0.0"))
    ui = Core.maintenanceUiArmOrConfirmUninstall(ui).ui
    compare(ui.uninstallArmed, true)
    ui = Core.maintenanceUiSetPurge(ui, true)
    compare(ui.purgeSettings, true)
    compare(ui.uninstallArmed, false)
  }

  function test_maintenance_intention_shapes() {
    var ui = Core.maintenanceUiIdle("10.0.0")
    ui.targetVersion = "10.1.0"
    var up = Core.maintenanceIntention("update_apply", ui)
    compare(up.kind, "update_apply")
    compare(up.version, "10.1.0")
    ui.purgeSettings = true
    var un = Core.maintenanceIntention("uninstall", ui)
    compare(un.kind, "uninstall")
    compare(un.purge, true)
    compare(un.payload.purgeSettingsAndBackups, true)
  }

  // ---- Source contracts ----

  function test_service_login_uses_exec_detached() {
    var src = read("assets/omarchy/Service.qml")
    verify(src.indexOf("Quickshell.execDetached") >= 0)
    verify(src.indexOf("loginDetachedArgv") >= 0)
    verify(src.indexOf("bash -lc") < 0)
    verify(src.indexOf("sh -c") < 0)
  }

  // BUNDLE-036 / UX-048: after writing uninstall confirmation, close stdin so the
  // helper's read_to_end receives EOF (write alone does not close the channel).
  function test_service_uninstall_stdin_closes_after_write() {
    var src = read("assets/omarchy/Service.qml")
    var handoff = src.indexOf("id: maintenanceHandoffProcess")
    verify(handoff >= 0)
    var onStarted = src.indexOf("onStarted:", handoff)
    verify(onStarted >= 0)
    var onExited = src.indexOf("onExited:", onStarted)
    verify(onExited > onStarted)
    var body = src.substring(onStarted, onExited)
    verify(body.indexOf("write(root.pendingMaintenancePayload") >= 0
           || body.indexOf("write(root.pendingMaintenancePayload +") >= 0
           || body.indexOf("pendingMaintenancePayload") >= 0)
    verify(body.indexOf("write(") >= 0)
    var writeAt = body.indexOf("write(")
    var closeAt = body.indexOf("stdinEnabled = false", writeAt)
    verify(closeAt > writeAt, "stdinEnabled=false must follow write for EOF")
  }

  function test_maintenance_view_ux_copy() {
    var src = read("assets/omarchy/MaintenanceView.qml")
    verify(src.indexOf("Check for updates") >= 0)
    verify(src.indexOf("Uninstall Agent Bar") >= 0)
    verify(src.indexOf("Also delete saved settings and backups") >= 0)
    verify(src.indexOf("ConfirmDialog") >= 0)
    verify(src.indexOf("Release notes") >= 0)
    verify(src.indexOf("Text.RichText") < 0)
    // §5.6: the package name is `agent-bar`; every surface says `Agent Bar`.
    verify(src.indexOf("Uninstall agent-bar") < 0)
    // The installation-type row is gone — it only ever had one value.
    verify(src.indexOf("Installation type") < 0)
    verify(src.indexOf("Plugin bundle") < 0)
    // Ceremony removed: no "Final confirmation:", no "Click Uninstall again."
    verify(src.indexOf("Final confirmation") < 0)
    verify(src.indexOf("Deletes Agent Bar, your settings and every backup.") >= 0)
    verify(src.indexOf("Deletes Agent Bar. Your settings stay.") >= 0)
    verify(src.indexOf("Removes Agent Bar. Your settings stay.") >= 0)
  }

  function test_install_type_is_gone_from_the_model() {
    var src = read("assets/omarchy/CoreMaintenance.js")
    // Dead the moment the row was deleted; the contract forbids keeping it.
    verify(src.indexOf("installType") < 0)
  }

  function test_settings_hosts_maintenance_view() {
    var src = read("assets/omarchy/SettingsView.qml")
    verify(src.indexOf("MaintenanceView") >= 0)
    verify(src.indexOf("land in the next task") < 0)
  }

  function test_helper_script_source_contract() {
    var src = read("scripts/agent-bar-open-terminal")
    verify(src.indexOf("xdg-terminal-exec") >= 0)
    verify(src.indexOf("--app-id=org.omarchy.terminal") >= 0)
    verify(src.indexOf("Agent Bar Login") >= 0)
    verify(src.indexOf("BASH_SOURCE") >= 0)
    verify(src.indexOf("cmd=\"$*\"") < 0)
    verify(src.indexOf("bash -lc") < 0)
    verify(src.indexOf("alacritty") < 0)
  }
}

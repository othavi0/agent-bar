import QtQuick
import QtTest
import "../../assets/omarchy/ServiceCore.js" as Core

TestCase {
  id: testCase
  name: "AgentBarSettingsRaces"
  when: windowShown

  // Harness mirrors Service settings lanes without Process I/O.
  Item {
    id: h
    property var settingsState: Core.settingsClosed()
    property var settingsDraft: null
    property var appliedSettings: null
    property bool settingsReadBusy: false
    property bool settingsWriteBusy: false
    property int settingsGeneration: 0
    property int activeSettingsReadGeneration: 0
    property int activeSettingsWriteGeneration: 0
    property string pendingSettingsPayload: ""
    property int settingsSaveCount: 0
    property var popupOwner: null
    property var maintenanceState: Core.maintenanceIdle()

    function openSettings(owner) {
      popupOwner = Core.requestPopup(popupOwner, owner, null, "settings")
      if (!settingsState || settingsState.phase === "closed") {
        settingsGeneration++
        activeSettingsReadGeneration = settingsGeneration
        settingsState = Core.settingsBeginLoad(settingsGeneration)
        settingsDraft = null
        settingsReadBusy = true
      }
    }

    function applyRead(generation, doc, exitCode) {
      if (!Core.shouldApplyGeneration(activeSettingsReadGeneration, generation))
        return
      settingsReadBusy = false
      if (exitCode !== 0 || !settingsState || settingsState.phase === "closed")
        return
      if (!Core.validateSettingsDraft(doc).ok)
        return
      settingsState = Core.settingsFinishLoad(settingsState, generation, doc)
      settingsDraft = settingsState.draft
      appliedSettings = doc
    }

    function mutate(mutator) {
      if (Core.settingsControlsLocked(settingsState))
        return
      settingsDraft = mutator(settingsDraft)
      settingsState = Core.settingsMarkDirty(settingsState)
      var next = Core.cloneState(settingsState)
      next.draft = settingsDraft
      settingsState = next
    }

    function save() {
      if (maintenanceState.blocked)
        return false
      if (!Core.settingsCanSave(settingsState, settingsDraft))
        return false
      if (!Core.canStartLane(settingsWriteBusy))
        return false
      var payloadObj = JSON.parse(JSON.stringify(settingsDraft))
      settingsGeneration++
      var gen = settingsGeneration
      pendingSettingsPayload = JSON.stringify(payloadObj)
      settingsState = Core.settingsBeginSave(settingsState, gen, payloadObj)
      settingsDraft = settingsState.draft
      activeSettingsWriteGeneration = gen
      settingsWriteBusy = true
      settingsSaveCount++
      return true
    }

    function applyWrite(generation, ok, canonical) {
      if (!Core.shouldApplyGeneration(activeSettingsWriteGeneration, generation))
        return
      settingsWriteBusy = false
      pendingSettingsPayload = ""
      settingsState = Core.settingsFinishSave(settingsState, generation, ok, canonical)
      settingsDraft = settingsState ? settingsState.draft : null
      if (ok && canonical)
        appliedSettings = canonical
    }

    function closePopup(owner) {
      popupOwner = Core.closePopup(popupOwner, owner)
      if (!popupOwner && !Core.settingsShouldRetainOnClose(settingsState)) {
        settingsState = Core.settingsClosed()
        settingsDraft = null
      }
    }

    function reset() {
      settingsState = Core.settingsClosed()
      settingsDraft = null
      appliedSettings = null
      settingsReadBusy = false
      settingsWriteBusy = false
      settingsGeneration = 0
      activeSettingsReadGeneration = 0
      activeSettingsWriteGeneration = 0
      pendingSettingsPayload = ""
      settingsSaveCount = 0
      popupOwner = null
      maintenanceState = Core.maintenanceIdle()
    }
  }

  function test_delayed_show_keeps_controls_locked() {
    h.reset()
    h.openSettings("mon-a")
    compare(h.settingsState.phase, "loading")
    compare(Core.settingsControlsLocked(h.settingsState), true)
    h.mutate(function (d) { return Core.setDisplayMetric(d || Core.defaultSettings(), "used") })
    compare(h.settingsDraft, null)
    h.applyRead(h.activeSettingsReadGeneration, Core.defaultSettings(), 0)
    compare(h.settingsState.phase, "clean")
    compare(Core.settingsControlsLocked(h.settingsState), false)
  }

  function test_stale_read_callback_ignored() {
    h.reset()
    h.openSettings("a")
    var gen1 = h.activeSettingsReadGeneration
    h.settingsReadBusy = false
    h.settingsState = Core.settingsClosed()
    h.openSettings("a")
    var gen2 = h.activeSettingsReadGeneration
    verify(gen2 > gen1)
    h.applyRead(gen1, Core.setDisplayMetric(Core.defaultSettings(), "used"), 0)
    compare(h.settingsState.phase, "loading")
    h.applyRead(gen2, Core.defaultSettings(), 0)
    compare(h.settingsState.phase, "clean")
    compare(h.settingsDraft.display.metric, "remaining")
  }

  function test_close_during_save_retains_busy_then_completes() {
    h.reset()
    h.openSettings("a")
    h.applyRead(h.activeSettingsReadGeneration, Core.defaultSettings(), 0)
    h.mutate(function (d) { return Core.setNotificationsEnabled(d, false) })
    compare(h.save(), true)
    var gen = h.activeSettingsWriteGeneration
    var payload = JSON.parse(h.pendingSettingsPayload)
    compare(h.settingsState.phase, "saving")
    h.closePopup("a")
    // SET-019: still saving after close
    compare(h.settingsState.phase, "saving")
    compare(h.settingsState.busy, true)
    h.applyWrite(gen, true, payload)
    compare(h.settingsState.phase, "clean")
    compare(h.appliedSettings.notifications.enabled, false)
  }

  function test_reopen_during_save_reflects_busy() {
    h.reset()
    h.openSettings("a")
    h.applyRead(h.activeSettingsReadGeneration, Core.defaultSettings(), 0)
    h.mutate(function (d) { return Core.setDisplayMetric(d, "used") })
    h.save()
    var gen = h.settingsGeneration
    h.openSettings("a")
    compare(h.settingsGeneration, gen)
    compare(h.settingsState.phase, "saving")
    compare(h.popupOwner.view, "settings")
  }

  function test_two_saves_second_rejected_while_busy() {
    h.reset()
    h.openSettings("a")
    h.applyRead(h.activeSettingsReadGeneration, Core.defaultSettings(), 0)
    h.mutate(function (d) { return Core.setRefreshInterval(d, 120) })
    compare(h.save(), true)
    compare(h.settingsSaveCount, 1)
    compare(h.save(), false)
    compare(h.settingsSaveCount, 1)
    var payload = JSON.parse(h.pendingSettingsPayload)
    h.applyWrite(h.activeSettingsWriteGeneration, true, payload)
    compare(h.settingsState.phase, "clean")
  }

  function test_stale_write_callback_ignored() {
    h.reset()
    h.openSettings("a")
    h.applyRead(h.activeSettingsReadGeneration, Core.defaultSettings(), 0)
    h.mutate(function (d) { return Core.setDisplayMetric(d, "used") })
    h.save()
    var gen1 = h.activeSettingsWriteGeneration
    var payload1 = JSON.parse(h.pendingSettingsPayload)
    h.applyWrite(gen1, true, payload1)
    compare(h.settingsState.phase, "clean")
    compare(h.settingsDraft.display.metric, "used")

    // Stale failure for gen1 must not dirty a completed save
    h.applyWrite(gen1, false, null)
    compare(h.settingsState.phase, "clean")
    compare(h.settingsDraft.display.metric, "used")
  }

  function test_second_save_generation_monotonic_and_stale_first_ignored() {
    h.reset()
    h.openSettings("a")
    h.applyRead(h.activeSettingsReadGeneration, Core.defaultSettings(), 0)

    h.mutate(function (d) { return Core.setDisplayMetric(d, "used") })
    h.save()
    var gen1 = h.activeSettingsWriteGeneration
    var payload1 = JSON.parse(h.pendingSettingsPayload)
    // Do not complete gen1 yet — but lane is busy so second save rejected.
    // Complete gen1 first, then start gen2, then late gen1 fails.
    h.applyWrite(gen1, true, payload1)

    h.mutate(function (d) { return Core.setRefreshInterval(d, 90) })
    h.save()
    var gen2 = h.activeSettingsWriteGeneration
    verify(gen2 > gen1)
    var payload2 = JSON.parse(h.pendingSettingsPayload)

    // Late gen1 failure must not affect gen2 save
    h.applyWrite(gen1, false, null)
    compare(h.settingsState.phase, "saving")
    h.applyWrite(gen2, true, payload2)
    compare(h.settingsState.phase, "clean")
    compare(h.settingsDraft.refreshIntervalSeconds, 90)
  }

  function test_payload_immutable_after_edit_during_save() {
    h.reset()
    h.openSettings("a")
    h.applyRead(h.activeSettingsReadGeneration, Core.defaultSettings(), 0)
    h.mutate(function (d) { return Core.setDisplayMetric(d, "used") })
    h.save()
    var captured = JSON.parse(h.pendingSettingsPayload)
    compare(captured.display.metric, "used")
    // Edits while saving are locked
    h.mutate(function (d) { return Core.setDisplayMetric(d, "remaining") })
    compare(JSON.parse(h.pendingSettingsPayload).display.metric, "used")
    compare(h.settingsState.pendingPayload.display.metric, "used")
  }
}

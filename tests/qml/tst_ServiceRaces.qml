import QtQuick
import QtTest
import "../../assets/omarchy/CoreService.js" as Core
import "../../assets/omarchy/CoreMaintenance.js" as Maintenance

TestCase {
  id: testCase
  name: "AgentBarServiceRaces"
  when: windowShown

  Item {
    id: h
    property var pendingForcedTargets: Core.emptyPending()
    property bool statusBusy: false
    property bool settingsReadBusy: false
    property bool settingsWriteBusy: false
    property bool maintenanceCheckBusy: false
    property int statusStartCount: 0
    property int settingsReadStartCount: 0
    property int settingsWriteStartCount: 0
    property int maintenanceCheckStartCount: 0
    property int statusGeneration: 0
    property int activeStatusGeneration: 0
    property var snapshot: null
    property var maintenanceState: Maintenance.maintenanceIdle()
    property bool versionReady: true
    property bool versionFailed: false
    property string helperVersion: "10.0.0"

    function kickStatus(forceAll) {
      if (!versionReady || versionFailed || maintenanceState.blocked)
        return false
      if (!Core.canStartLane(statusBusy))
        return false
      if (forceAll)
        pendingForcedTargets = Core.unionForced(pendingForcedTargets, "all")
      statusGeneration++
      activeStatusGeneration = statusGeneration
      var taken = Core.takePending(pendingForcedTargets)
      pendingForcedTargets = taken.remaining
      statusBusy = true
      statusStartCount++
      lastForced = taken.captured
      return true
    }
    property var lastForced: null

    function kickSettingsRead() {
      // ARCH-024: handoff rejects new writes/polling; reads may still start.
      if (!Core.canStartLane(settingsReadBusy))
        return false
      settingsReadBusy = true
      settingsReadStartCount++
      return true
    }

    function kickSettingsWrite() {
      if (!Maintenance.maintenanceCanStartWrite(maintenanceState))
        return false
      if (!Core.canStartLane(settingsWriteBusy))
        return false
      settingsWriteBusy = true
      settingsWriteStartCount++
      return true
    }

    function kickMaintenanceCheck() {
      if (!Core.canStartLane(maintenanceCheckBusy))
        return false
      maintenanceCheckBusy = true
      maintenanceCheckStartCount++
      return true
    }

    function finishStatus(gen, applySnapshot) {
      if (!Core.shouldApplyGeneration(activeStatusGeneration, gen))
        return
      statusBusy = false
      if (applySnapshot)
        snapshot = { schemaVersion: 2, n: statusStartCount }
      if (!Core.pendingIsEmpty(pendingForcedTargets))
        kickStatus(false)
    }
  }

  function reset() {
    h.pendingForcedTargets = Core.emptyPending()
    h.statusBusy = false
    h.settingsReadBusy = false
    h.settingsWriteBusy = false
    h.maintenanceCheckBusy = false
    h.statusStartCount = 0
    h.settingsReadStartCount = 0
    h.settingsWriteStartCount = 0
    h.maintenanceCheckStartCount = 0
    h.statusGeneration = 0
    h.activeStatusGeneration = 0
    h.snapshot = null
    h.maintenanceState = Maintenance.maintenanceIdle()
    h.versionReady = true
    h.versionFailed = false
  }

  function test_coalesce_forced_targets_while_status_busy() {
    reset()
    compare(h.kickStatus(false), true)
    compare(h.statusStartCount, 1)
    // Overlapping forces while busy → union, no second start.
    h.pendingForcedTargets = Core.unionForced(h.pendingForcedTargets, "claude")
    h.pendingForcedTargets = Core.unionForced(h.pendingForcedTargets, "amp")
    compare(h.kickStatus(false), false)
    compare(h.statusStartCount, 1)
    // Complete → follow-up with unioned targets.
    h.finishStatus(h.activeStatusGeneration, true)
    compare(h.statusStartCount, 2)
    verify(h.lastForced.ids["claude"] === true)
    verify(h.lastForced.ids["amp"] === true)
  }

  function test_all_dominates_pending_union() {
    reset()
    compare(h.kickStatus(false), true)
    h.pendingForcedTargets = Core.unionForced(h.pendingForcedTargets, "claude")
    h.pendingForcedTargets = Core.unionForced(h.pendingForcedTargets, "all")
    h.finishStatus(h.activeStatusGeneration, true)
    compare(h.statusStartCount, 2)
    compare(h.lastForced.all, true)
  }

  function test_lanes_overlap_without_cancel() {
    reset()
    compare(h.kickStatus(false), true)
    compare(h.kickSettingsRead(), true)
    compare(h.kickSettingsWrite(), true)
    compare(h.kickMaintenanceCheck(), true)
    // Same-lane re-entry blocked.
    compare(h.kickStatus(false), false)
    compare(h.kickSettingsRead(), false)
    compare(h.kickSettingsWrite(), false)
    compare(h.kickMaintenanceCheck(), false)
    // Counts: one each.
    compare(h.statusStartCount, 1)
    compare(h.settingsReadStartCount, 1)
    compare(h.settingsWriteStartCount, 1)
    compare(h.maintenanceCheckStartCount, 1)
  }

  function test_maintenance_blocks_new_writes_and_status() {
    reset()
    h.maintenanceState = Maintenance.maintenanceBeginHandoff(h.maintenanceState)
    compare(h.kickStatus(false), false)
    compare(h.kickSettingsWrite(), false)
    // Read/check may still be allowed by policy; write/status blocked.
    compare(h.kickSettingsRead(), true)
  }

  function test_maintenance_detach_after_mutable_drain() {
    reset()
    h.statusBusy = true
    h.settingsWriteBusy = true
    h.maintenanceState = Maintenance.maintenanceBeginHandoff(h.maintenanceState)
    compare(Maintenance.maintenanceCanDetach(h.maintenanceState, h.statusBusy, h.settingsWriteBusy), false)
    h.statusBusy = false
    compare(Maintenance.maintenanceCanDetach(h.maintenanceState, h.statusBusy, h.settingsWriteBusy), false)
    h.settingsWriteBusy = false
    compare(Maintenance.maintenanceCanDetach(h.maintenanceState, h.statusBusy, h.settingsWriteBusy), true)
  }

  function test_stale_status_callback_does_not_apply() {
    reset()
    compare(h.kickStatus(false), true)
    var gen1 = h.activeStatusGeneration
    h.statusBusy = false
    compare(h.kickStatus(false), true)
    var gen2 = h.activeStatusGeneration
    verify(gen2 > gen1)
    h.finishStatus(gen1, true)
    // Stale gen1 must not set snapshot when gen2 is active.
    // finishStatus checks generation — snapshot stays null until gen2 applies.
    compare(h.snapshot, null)
    h.finishStatus(gen2, true)
    verify(h.snapshot !== null)
  }

  function test_provider_force_then_all_while_busy() {
    reset()
    compare(h.kickStatus(false), true)
    h.pendingForcedTargets = Core.unionForced(h.pendingForcedTargets, "grok")
    h.pendingForcedTargets = Core.unionForced(h.pendingForcedTargets, "all")
    h.pendingForcedTargets = Core.unionForced(h.pendingForcedTargets, "claude")
    h.finishStatus(h.activeStatusGeneration, false)
    compare(h.lastForced.all, true)
  }
}

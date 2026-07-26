import QtQuick
import Quickshell.Io
import "ServiceCore.js" as Core

// Shared Agent Bar service (one instance per shell).
// Task 8 skeleton: versionProbe + health/refresh IPC. Full polling is Task 9.
Item {
  id: root

  // Injected by omarchy-shell / Quattro service loader (exact contract).
  property string omarchyPath: ""
  property var shell: null
  property var manifest: null
  property var barWidgetRegistry: null
  property var pluginRegistry: null

  readonly property string pluginRoot: manifest && manifest.__sourceDir
      ? String(manifest.__sourceDir)
      : ""

  // Test harness: absolute path to private helper. Production uses pluginRoot/bin.
  property string helperPath: ""

  // When true, skip auto Process probe; tests call applyVersionProbeResult().
  property bool testMode: false

  property int versionProbeTimeoutMs: 2000
  property string helperVersion: ""
  property bool versionReady: false
  property bool versionFailed: false
  property bool versionProbeRunning: false

  // Deliberate delay before collection (cold-start ordering tests).
  property int collectionDelayMs: 0
  property bool collectionStarted: false

  property int refreshRequestCount: 0
  property string lastRefreshProviderId: ""
  property var pendingForcedTargets: ({})

  readonly property string manifestVersion: manifest && manifest.version
      ? String(manifest.version)
      : ""

  function resolvedHelperPath() {
    if (helperPath && helperPath.length > 0)
      return helperPath
    if (pluginRoot && pluginRoot.length > 0)
      return pluginRoot + "/bin/agent-bar"
    return ""
  }

  function health(expectedVersion) {
    return Core.health(versionReady, versionFailed, helperVersion, manifestVersion, expectedVersion)
  }

  function refresh(providerId) {
    var result = Core.refreshResult(providerId)
    if (result !== "ok")
      return result
    lastRefreshProviderId = String(providerId)
    refreshRequestCount++
    pendingForcedTargets = Core.queueForcedProvider(pendingForcedTargets, providerId)
    return "ok"
  }

  // Used by tests and by Process onExited.
  function applyVersionProbeResult(stdout, stderr, exitCode) {
    var version = Core.parseVersionStdout(stdout, stderr, exitCode)
    if (version)
      finishVersionProbeSuccess(version)
    else
      finishVersionProbeFailure()
  }

  function startVersionProbe() {
    if (versionProbeRunning || versionReady)
      return
    if (testMode)
      return
    var helper = resolvedHelperPath()
    if (!helper || helper.length === 0) {
      finishVersionProbeFailure()
      return
    }
    versionProbeRunning = true
    versionFailed = false
    versionOut.text = ""
    versionErr.text = ""
    versionProbe.command = [helper, "version"]
    versionProbe.running = true
    versionTimeout.restart()
  }

  function finishVersionProbeSuccess(versionText) {
    versionTimeout.stop()
    versionProbeRunning = false
    helperVersion = versionText
    versionReady = true
    versionFailed = false
    if (collectionDelayMs > 0) {
      collectionDelay.interval = collectionDelayMs
      collectionDelay.start()
    } else {
      collectionStarted = true
    }
  }

  function finishVersionProbeFailure() {
    versionTimeout.stop()
    versionProbeRunning = false
    versionReady = false
    versionFailed = true
    helperVersion = ""
  }

  Process {
    id: versionProbe
    stdout: StdioCollector {
      id: versionOut
      waitForEnd: true
    }
    stderr: StdioCollector {
      id: versionErr
      waitForEnd: true
    }
    onExited: function (exitCode) {
      if (!root.versionProbeRunning)
        return
      root.applyVersionProbeResult(versionOut.text || "", versionErr.text || "", exitCode)
    }
  }

  Timer {
    id: versionTimeout
    interval: root.versionProbeTimeoutMs
    repeat: false
    onTriggered: {
      if (!root.versionProbeRunning)
        return
      if (versionProbe.running)
        versionProbe.running = false
      root.finishVersionProbeFailure()
    }
  }

  Timer {
    id: collectionDelay
    repeat: false
    onTriggered: root.collectionStarted = true
  }

  IpcHandler {
    target: "agent-bar.usage"

    function health(expectedVersion): string {
      return root.health(expectedVersion)
    }

    function refresh(providerId): string {
      return root.refresh(providerId)
    }
  }

  Component.onCompleted: startVersionProbe()
}

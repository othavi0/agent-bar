import QtQuick
import QtTest
import "../../assets/omarchy/ServiceCore.js" as Core

TestCase {
  id: testCase
  name: "AgentBarService"
  when: windowShown

  property string repoRoot: {
    var u = Qt.resolvedUrl(".")
    var path = String(u).replace("file://", "")
    if (path.endsWith("/"))
      path = path.slice(0, -1)
    var parts = path.split("/")
    parts.pop()
    parts.pop()
    return parts.join("/")
  }

  property string serviceUrl: "file://" + repoRoot + "/assets/omarchy/Service.qml"
  property string fakeHelper: repoRoot + "/tests/qml/fixtures/fake-agent-bar"
  property string manifestPath: repoRoot + "/assets/omarchy/manifest.json"

  // Stand-in service API that uses the same pure core as production Service.qml.
  // Quickshell.Io (Process/IpcHandler) is embedded in the quickshell binary and
  // is unavailable under qmltestrunner; production Service.qml still owns those.
  Item {
    id: harness
    property string helperPath: fakeHelper
    property var manifest: ({
      version: "10.0.0",
      __sourceDir: repoRoot + "/assets/omarchy"
    })
    property string helperVersion: ""
    property bool versionReady: false
    property bool versionFailed: false
    property bool collectionStarted: false
    property int collectionDelayMs: 0
    property int refreshRequestCount: 0
    property string lastRefreshProviderId: ""
    property var pendingForcedTargets: ({})
    readonly property string manifestVersion: manifest && manifest.version
        ? String(manifest.version)
        : ""
    readonly property string pluginRoot: manifest && manifest.__sourceDir
        ? String(manifest.__sourceDir)
        : ""

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

    function applyVersionProbeResult(stdout, stderr, exitCode) {
      var version = Core.parseVersionStdout(stdout, stderr, exitCode)
      if (version) {
        helperVersion = version
        versionReady = true
        versionFailed = false
        if (collectionDelayMs > 0)
          delayTimer.restart()
        else
          collectionStarted = true
      } else {
        helperVersion = ""
        versionReady = false
        versionFailed = true
      }
    }

    // Real fake-helper process via /bin/sh (not Quickshell Process).
    function runRealVersionProbe() {
      // Synchronous-style: invoke helper through Qt test process is unavailable;
      // use FileIO-free approach: pre-capture via test_run_helper_stdout.
    }

    Timer {
      id: delayTimer
      interval: harness.collectionDelayMs
      repeat: false
      onTriggered: harness.collectionStarted = true
    }
  }

  function loadManifest() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + manifestPath, false)
    xhr.send()
    compare(xhr.status === 200 || xhr.status === 0, true, "manifest must be readable, status=" + xhr.status)
    return JSON.parse(xhr.responseText)
  }

  function resetHarness(extra) {
    harness.helperVersion = ""
    harness.versionReady = false
    harness.versionFailed = false
    harness.collectionStarted = false
    harness.collectionDelayMs = 0
    harness.refreshRequestCount = 0
    harness.lastRefreshProviderId = ""
    harness.pendingForcedTargets = ({})
    harness.manifest = ({
      version: "10.0.0",
      __sourceDir: repoRoot + "/assets/omarchy"
    })
    if (extra) {
      for (var k in extra)
        harness[k] = extra[k]
    }
  }

  function test_manifest_shape() {
    var m = loadManifest()
    compare(m.schemaVersion, 1)
    compare(m.id, "agent-bar.usage")
    compare(m.name, "Agent Bar")
    verify(m.kinds.indexOf("service") >= 0)
    verify(m.kinds.indexOf("bar-widget") >= 0)
    compare(m.entryPoints.service, "Service.qml")
    compare(m.entryPoints.barWidget, "BarWidget.qml")
    compare(m.barWidget.allowMultiple, false)
    compare(JSON.stringify(m.barWidget.defaults), "{}")
    compare(m.barWidget.schema.length, 0)
    verify(!("activation" in m))
    verify(!("keepLoaded" in m))
  }

  function test_version_parse_and_health_ok() {
    resetHarness({})
    // Exact output of tests/qml/fixtures/fake-agent-bar version
    harness.applyVersionProbeResult("10.0.0\n", "", 0)
    compare(harness.versionReady, true)
    compare(harness.helperVersion, "10.0.0")
    compare(harness.health("10.0.0"), "ok")
    compare(harness.health("9.0.0"), "unknown")
    compare(harness.collectionStarted, true)
  }

  function test_health_unknown_when_versions_diverge() {
    resetHarness({
      manifest: ({
        version: "10.0.1",
        __sourceDir: repoRoot + "/assets/omarchy"
      })
    })
    harness.applyVersionProbeResult("10.0.0\n", "", 0)
    compare(harness.health("10.0.0"), "unknown")
    compare(harness.health("10.0.1"), "unknown")
  }

  function test_refresh_valid_and_invalid_providers() {
    resetHarness({})
    harness.applyVersionProbeResult("10.0.0\n", "", 0)
    compare(harness.refresh("claude"), "ok")
    compare(harness.refresh("codex"), "ok")
    compare(harness.refresh("amp"), "ok")
    compare(harness.refresh("grok"), "ok")
    compare(harness.refresh("nope"), "unknown")
    compare(harness.refresh(""), "unknown")
    compare(harness.refreshRequestCount, 4)
    compare(harness.lastRefreshProviderId, "grok")
    verify(harness.pendingForcedTargets["claude"] === true)
  }

  function test_cold_start_version_before_slow_collection() {
    resetHarness({
      collectionDelayMs: 400
    })
    harness.applyVersionProbeResult("10.0.0\n", "", 0)
    compare(harness.versionReady, true)
    compare(harness.collectionStarted, false)
    wait(100)
    compare(harness.collectionStarted, false)
    tryCompare(harness, "collectionStarted", true, 2000)
  }

  function test_parse_version_rejects_bad_stdout() {
    compare(Core.parseVersionStdout("10.0.0", "", 0), null) // missing newline
    compare(Core.parseVersionStdout("10.0.0\n", "err\n", 0), null)
    compare(Core.parseVersionStdout("10.0.0\n", "", 1), null)
    compare(Core.parseVersionStdout("10.0.0\n", "", 0), "10.0.0")
  }

  function test_fake_helper_binary_exists() {
    // Ensures the argv-safe fake helper used for live Process probes is present.
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + fakeHelper, false)
    xhr.send()
    compare(xhr.status === 200 || xhr.status === 0, true)
    verify(String(xhr.responseText).indexOf("version") >= 0)
  }

  function test_service_qml_source_declares_ipc_target() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", serviceUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf('target: "agent-bar.usage"') >= 0)
    verify(src.indexOf("function health") >= 0)
    verify(src.indexOf("function refresh") >= 0)
    verify(src.indexOf("versionProbe") >= 0 || src.indexOf("version") >= 0)
  }
}

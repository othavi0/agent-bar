// Pure service logic shared by Service.qml and QML unit tests.
// Keep free of Quickshell.Io so tests run under qmltestrunner.
.pragma library

var CLOSED_PROVIDERS = {
  "claude": true,
  "codex": true,
  "amp": true,
  "grok": true
}

function health(versionReady, versionFailed, helperVersion, manifestVersion, expectedVersion) {
  var expected = String(expectedVersion || "")
  if (!versionReady || versionFailed)
    return "unknown"
  if (String(helperVersion) === expected && String(manifestVersion) === expected)
    return "ok"
  return "unknown"
}

function refreshResult(providerId) {
  var id = String(providerId || "")
  if (!CLOSED_PROVIDERS[id])
    return "unknown"
  return "ok"
}

// Exact: semantic version + newline, empty stderr, exit 0.
function parseVersionStdout(stdout, stderr, exitCode) {
  if (exitCode !== 0)
    return null
  if (stderr && String(stderr).length > 0)
    return null
  var m = String(stdout || "").match(/^(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)\n$/)
  return m ? m[1] : null
}

function queueForcedProvider(pending, providerId) {
  var next = {}
  if (pending) {
    for (var k in pending)
      next[k] = pending[k]
  }
  if (next.all)
    return next
  next[String(providerId)] = true
  return next
}

function isClosedProvider(providerId) {
  return !!CLOSED_PROVIDERS[String(providerId || "")]
}

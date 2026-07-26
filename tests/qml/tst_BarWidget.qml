import QtQuick
import QtTest

TestCase {
  id: testCase
  name: "AgentBarBarWidget"
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

  property string widgetUrl: "file://" + repoRoot + "/assets/omarchy/BarWidget.qml"

  // Minimal shell stand-in with Quattro serviceFor API.
  Item {
    id: fakeShell
    property var _services: ({})

    function serviceFor(pluginId) {
      return _services[String(pluginId)] || null
    }

    function registerService(pluginId, svc) {
      var next = ({})
      for (var k in _services)
        next[k] = _services[k]
      next[String(pluginId)] = svc
      _services = next
    }
  }

  Item {
    id: fakeBar
    property var shell: fakeShell
    property color foreground: "#ffffff"
    property string fontFamily: "monospace"
    property bool vertical: false
    property int barSize: 28
  }

  // Chip logic matching BarWidget.qml agentService resolution (without qs.Ui).
  component AgentChip: Item {
    property var bar: null
    property string moduleName: "agent-bar.usage"
    readonly property var agentService: bar && bar.shell
        ? bar.shell.serviceFor(moduleName)
        : null
  }

  function test_two_widgets_resolve_same_service() {
    var svc = Qt.createQmlObject('import QtQuick; Item { property string helperVersion: "10.0.0"; property bool versionReady: true; property bool versionFailed: false }', testCase)
    fakeShell.registerService("agent-bar.usage", svc)

    var w1 = agentChipComp.createObject(testCase, {
      bar: fakeBar,
      moduleName: "agent-bar.usage"
    })
    var w2 = agentChipComp.createObject(testCase, {
      bar: fakeBar,
      moduleName: "agent-bar.usage"
    })
    verify(w1.agentService !== null)
    verify(w2.agentService !== null)
    compare(w1.agentService, svc)
    compare(w2.agentService, svc)
    compare(w1.agentService, w2.agentService)
    compare(w1.agentService.helperVersion, "10.0.0")

    w1.destroy()
    w2.destroy()
    svc.destroy()
  }

  function test_widget_without_shell_has_null_service() {
    var w = agentChipComp.createObject(testCase, {
      bar: null,
      moduleName: "agent-bar.usage"
    })
    compare(w.agentService, null)
    w.destroy()
  }

  function test_bar_widget_source_uses_serviceFor() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", widgetUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf("serviceFor(moduleName)") >= 0)
    verify(src.indexOf("moduleName: \"agent-bar.usage\"") >= 0)
    verify(src.indexOf("Qt.resolvedUrl") < 0)
  }

  Component {
    id: agentChipComp
    AgentChip {}
  }
}

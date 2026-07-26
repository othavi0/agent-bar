import QtQuick
import qs.Ui
import qs.Commons

// Monitor-local bar chip. Resolves the shared service via shell.serviceFor.
BarWidget {
  id: root
  moduleName: "agent-bar.usage"

  readonly property var agentService: bar && bar.shell
      ? bar.shell.serviceFor(moduleName)
      : null

  readonly property string labelText: {
    if (!agentService)
      return "agent-bar"
    if (agentService.versionFailed)
      return "agent-bar!"
    if (!agentService.versionReady)
      return "agent-bar…"
    return "agent-bar " + String(agentService.helperVersion || "")
  }

  implicitWidth: chip.implicitWidth + Style.space(12)
  implicitHeight: barSize

  Text {
    id: chip
    anchors.centerIn: parent
    text: root.labelText
    color: root.bar ? root.bar.foreground : Color.foreground
    font.family: root.bar ? root.bar.fontFamily : "monospace"
    font.pixelSize: Style.font.body
  }
}

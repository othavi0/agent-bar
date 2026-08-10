import QtQuick
import qs.Commons
import qs.Ui

// Provider content header — name, plan tag, severity tag, refresh only
// (Fase 2 slim-down). Connection state is implied structurally (UX-017);
// last-success age appears in the pane's own age caption, not here.
// UX-016: deliberately no provider icon here (icon lives only on the rail).
Item {
  id: root

  property string name: ""
  property string plan: ""
  property bool refreshing: false
  property string severityText: ""
  property bool severityUrgent: false
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  signal refreshClicked()

  width: parent ? parent.width : implicitWidth
  implicitHeight: row.implicitHeight
  height: implicitHeight

  Row {
    id: row
    width: Math.max(0, parent.width)
    spacing: Style.space(8)

    Text {
      id: nameLabel
      // Cap name so the row never forces content wider than the pane.
      width: Math.min(implicitWidth, Math.max(Style.space(48), parent.width * 0.42))
      text: root.name
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.name: root.name
      Accessible.role: Accessible.Heading
    }

    HeaderTag {
      id: planTag
      anchors.verticalCenter: parent.verticalCenter
      label: root.plan
      accessibleName: "plan " + root.plan
      foreground: root.foreground
      fontFamily: root.fontFamily
    }

    HeaderTag {
      id: severityTag
      anchors.verticalCenter: parent.verticalCenter
      label: root.severityText
      urgent: root.severityUrgent
      accessibleName: "severity " + root.severityText
      foreground: root.foreground
      fontFamily: root.fontFamily
    }

    Item {
      // Flexible spacer; never negative. Pushes the refresh glyph right.
      // Subtracts what is actually rendered — an invisible tag takes no
      // room in the Row, so it must take none here either.
      width: Math.max(Style.space(4),
          parent.width
          - nameLabel.width
          - (planTag.visible ? planTag.width + row.spacing : 0)
          - (severityTag.visible ? severityTag.width + row.spacing : 0)
          - Style.space(22)
          - row.spacing * 2)
      height: 1
    }

    // UX-051 refresh glyph
    PanelActionButton {
      size: Style.space(22)
      iconText: "󰑐"
      tooltipText: root.refreshing ? "Refreshing…" : "Refresh provider"
      foreground: root.foreground
      enabled: !root.refreshing
      focusable: true
      Accessible.name: "Refresh provider"
      onClicked: root.refreshClicked()
    }
  }
}

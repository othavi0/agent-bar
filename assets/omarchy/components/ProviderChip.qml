import QtQuick
import QtQuick.Effects
import qs.Commons
import qs.Ui

// Bar chip on Quattro's WidgetButton (UX-001..012; visual design §5).
// WidgetButton owns the click-target registration and triggerPress
// protocol (UX-010), pointer cursor, dimmed/concealed semantics, and the
// host-owned opacity motion (A11Y-013). Wheel events die unconnected
// inside the host component (UX-009).
WidgetButton {
  id: root

  property string providerId: ""
  property string displayName: ""
  property string numeralText: "—"
  property string stateCue: ""
  property url iconSource: ""
  property bool tinted: false
  property real iconScale: 1.0
  property bool severityUrgent: false
  property string cueLabel: ""

  // The host tooltip is intentionally never set; this label exists for
  // assistive tech only and must stay single-line.
  property string accessibleLabel: ""

  // §7: Color.urgent is the single severity colour; no new colour exists.
  readonly property color numeralColor: root.severityUrgent ? Color.urgent : root.foreground

  readonly property int paintedIconSize: Math.round(Style.bar.iconCanvas * iconScale)

  labelVisible: false
  hasVisualContent: true
  fixedWidth: vertical ? -1 : content.implicitWidth
  fixedHeight: vertical ? Style.bar.iconSlot : -1

  Row {
    id: content
    anchors.centerIn: parent
    spacing: Style.spacing.sm

    Item {
      width: Style.bar.iconCanvas
      height: Style.bar.iconCanvas
      anchors.verticalCenter: parent.verticalCenter

      Image {
        id: mark
        source: root.tinted ? "" : root.iconSource
        visible: !root.tinted && source.toString().length > 0
        anchors.centerIn: parent
        width: root.paintedIconSize
        height: root.paintedIconSize
        sourceSize.width: root.paintedIconSize
        sourceSize.height: root.paintedIconSize
        fillMode: Image.PreserveAspectFit
        Accessible.name: root.displayName
        Accessible.role: Accessible.Graphic
      }

      Image {
        id: tintSource
        source: root.tinted ? root.iconSource : ""
        visible: false
        anchors.centerIn: parent
        width: root.paintedIconSize
        height: root.paintedIconSize
        sourceSize.width: root.paintedIconSize
        sourceSize.height: root.paintedIconSize
        fillMode: Image.PreserveAspectFit
      }

      // Measured fact: colorization multiplies mask luminance by
      // colorizationColor, preserving alpha — white masks render exactly
      // in colorizationColor (proven on the GPU backend; the offscreen
      // software backend renders no shader at all).
      MultiEffect {
        visible: root.tinted
        source: tintSource
        anchors.fill: tintSource
        colorization: 1.0
        colorizationColor: root.foreground
        Accessible.name: root.displayName
        Accessible.role: Accessible.Graphic
      }
    }

    Text {
      visible: !root.vertical
      text: root.numeralText
      color: root.numeralColor
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      anchors.verticalCenter: parent.verticalCenter
      renderType: Text.NativeRendering
      textFormat: Text.PlainText
      Accessible.name: root.accessibleLabel
      Accessible.role: Accessible.StaticText
    }

    Text {
      visible: !root.vertical && root.stateCue.length > 0
      text: root.stateCue
      color: root.numeralColor
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      anchors.verticalCenter: parent.verticalCenter
      renderType: Text.NativeRendering
      textFormat: Text.PlainText
      Accessible.name: root.cueLabel.length > 0 ? root.cueLabel : root.stateCue
      Accessible.role: Accessible.StaticText
    }
  }

  // §5: the numeral box is tight — width follows the text (amended
  // 2026-08-04). The reserved box sized on the widest numeral parked ~2
  // digits of slack in the inter-chip gap when every numeral was short,
  // reading as disproportionate spacing with an inflated right edge. Chips
  // may shift when a value crosses a digit boundary; the state cues (! / ln)
  // already resized them, so width stability was never absolute.
}

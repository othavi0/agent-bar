import QtQuick
import qs.Commons

// One normalized percentage window. The elected lead renders large (label
// line with the promoted reset -> 2.5x numeral + unit -> track); every other
// window renders as a compact row that carries its own track (UX-020A
// extended by the visual design §6). Severity paints numeral and fill in
// Color.urgent (§7) and always travels with a word — see Accessible.name.
Item {
  id: root

  property string label: ""
  property string percentText: "—"
  // 0–100 when known; negative when unavailable (hide fill).
  property real percent: -1
  // Countdown only: the popup shows no absolute clock (§6).
  property string resetCountdown: ""
  // Lead-only wall-clock suffix, already parenthesised ("(13:31)"); compact
  // rows leave it empty (§6 amendment 2026-08-03).
  property string resetClock: ""
  property string resetPhrase: ""
  property string unitText: "left"
  // "critical" | "warning" | "" — computed from usedPercent by CoreView.
  property string severity: ""
  // The elected lead renders large; every other window renders compact.
  property bool emphasis: true
  property bool dimmed: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  readonly property bool hasPercent: root.percent >= 0 && root.percent <= 100
  readonly property real fillRatio: hasPercent
      ? Math.max(0, Math.min(1, root.percent / 100))
      : 0
  readonly property bool isCritical: root.severity === "critical"
  readonly property color valueColor: root.isCritical ? Color.urgent : root.foreground
  // Severity describes the numbers on screen, so it outranks dimming: a
  // window still shows its reading, and a critical reading is still
  // critical. valueColor and fillColor must agree on this; they disagreed
  // once, and the numeral and its own track rendered two different verdicts.
  // UX-028 (amended) retired the one caller that ever set `dimmed` — stale
  // no longer dims anything. The property stays because it is the component's
  // own low-emphasis mode, not a staleness signal; no caller sets it today.
  readonly property color fillColor: root.isCritical
      ? Color.urgent
      : (root.dimmed ? root.foreground : root.accent)
  readonly property real fillOpacity: root.dimmed ? 0.45 : (root.isCritical ? 1.0 : 0.9)
  // Data surface, not control chrome — no host token covers it. Declared
  // once here; both layouts tint from the same place.
  readonly property color trackColor: Util.alpha(root.foreground, 0.12)

  width: parent ? parent.width : implicitWidth
  implicitHeight: root.emphasis ? bigCol.implicitHeight : compactRow.implicitHeight
  height: implicitHeight
  opacity: root.dimmed ? 0.6 : 1.0

  // §6: the compact reset column is sized for the widest countdown the
  // humaniser produces below 24 hours — countdownText() pads no digits, so
  // two-digit hours plus two-digit minutes is the worst case — and the
  // value column for a full 100%. Never a hardcoded pixel: both scale with
  // [font] base-size.
  TextMetrics {
    id: countdownMetrics
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    text: "23h 59m"
  }

  TextMetrics {
    id: valueMetrics
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
    text: "100%"
  }

  Column {
    id: bigCol
    visible: root.emphasis
    width: parent.width
    spacing: Style.spacing.sm

    // Label line with the reset promoted into it: the label and the lead-in
    // recede, the countdown itself keeps full ink. Not uppercased — this line
    // is now a sentence, not a kicker.
    Row {
      width: parent.width
      spacing: Style.spacing.sm

      Text {
        id: leadLabel
        width: Math.min(implicitWidth,
                        Math.max(0, parent.width - leadReset.implicitWidth - Style.spacing.sm))
        text: root.resetPhrase.length > 0
            ? root.label + " · " + root.resetPhrase
            : root.label
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        elide: Text.ElideRight
        textFormat: Text.PlainText
        Accessible.ignored: true
      }

      Text {
        id: leadReset
        text: root.resetClock.length > 0
            ? root.resetCountdown + " " + root.resetClock
            : root.resetCountdown
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }

    Row {
      spacing: Style.spacing.md

      Text {
        id: bigNumeral
        text: root.percentText
        color: root.valueColor
        font.family: root.fontFamily
        font.pixelSize: Math.round(Style.font.body * 2.5)
        font.bold: true
        textFormat: Text.PlainText
        Accessible.ignored: true
      }

      Text {
        anchors.baseline: bigNumeral.baseline
        text: root.unitText
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }

    Rectangle {
      width: parent.width
      height: Style.spacing.md
      radius: height / 2
      color: root.trackColor
      Accessible.ignored: true

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.hasPercent && root.fillRatio > 0 ? Style.spacing.md : 0,
                        parent.width * root.fillRatio)
        height: parent.height
        radius: parent.radius
        color: root.fillColor
        opacity: root.fillOpacity
        visible: root.hasPercent && root.fillRatio > 0
      }
    }
  }

  Row {
    id: compactRow
    visible: !root.emphasis
    width: parent.width
    spacing: Style.spacing.lg

    Text {
      id: compactLabel
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(0, Math.round(parent.width * 0.25))
      text: root.label
      color: Util.alpha(root.foreground, 0.72)
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.ignored: true
    }

    Rectangle {
      id: compactTrack
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(0, parent.width
                         - compactLabel.width
                         - compactValue.width
                         - compactReset.width
                         - Style.spacing.lg * 3)
      height: Style.spacing.sm
      radius: height / 2
      color: root.trackColor
      Accessible.ignored: true

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.hasPercent && root.fillRatio > 0 ? Style.spacing.sm : 0,
                        parent.width * root.fillRatio)
        height: parent.height
        radius: parent.radius
        color: root.fillColor
        opacity: root.fillOpacity
        visible: root.hasPercent && root.fillRatio > 0
      }
    }

    Text {
      id: compactValue
      anchors.verticalCenter: parent.verticalCenter
      width: valueMetrics.advanceWidth
      horizontalAlignment: Text.AlignRight
      text: root.percentText
      color: root.valueColor
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      font.bold: true
      textFormat: Text.PlainText
      Accessible.ignored: true
    }

    Text {
      id: compactReset
      anchors.verticalCenter: parent.verticalCenter
      width: countdownMetrics.advanceWidth
      horizontalAlignment: Text.AlignRight
      text: root.resetCountdown
      color: Util.alpha(root.foreground, 0.55)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      textFormat: Text.PlainText
      Accessible.ignored: true
    }
  }

  // A11Y-012: severity reaches assistive tech as a word, in both layouts.
  Accessible.name: {
    var parts = [root.label, root.percentText + " " + root.unitText]
    if (root.severity === "critical")
      parts.push("critical")
    else if (root.severity === "warning")
      parts.push("low")
    if (root.resetCountdown.length > 0)
      parts.push(root.resetClock.length > 0
          ? root.resetPhrase + " " + root.resetCountdown + " " + root.resetClock
          : root.resetPhrase + " " + root.resetCountdown)
    return parts.join(", ")
  }
  Accessible.role: Accessible.StaticText
}

import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.Commons
import qs.Ui

// agent-bar — chips de quota por provider + popup nativo (usage + settings).
// Fonte de dados: `agent-bar --format json` (envelope schemaVersion 1,
// ver docs/json-output.md no repo do agent-bar). Settings editáveis via
// `agent-bar config show|apply`. Este arquivo é escrito pelo
// `agent-bar setup` e version-locked com o binário.
BarWidget {
  id: root
  moduleName: "agent-bar.usage"

  property bool popupOpen: false
  property var payload: null
  property bool stale: false
  property bool forceNext: false

  property bool settingsMode: false
  property var draftSettings: ({
    providers: [],
    providerOrder: [],
    displayMode: "remaining",
    notifyEnabled: true,
    refreshIntervalSec: 60
  })
  // null = config show ainda não retornou → sem filtro (mostra todos)
  property var enabledIds: null
  property string displayMode: "remaining"
  property string settingsStatusText: ""
  property bool settingsBusy: false
  // Lido de `config show` (contrato do Plano 02) — gate único de motion
  // (M1/M2/M4). Ausente no payload = true (nunca bloqueia motion à toa).
  property bool menuAnimationsEnabled: true
  // true só durante a janela de abertura do popup — M1 é "abertura", não
  // "toda atualização"; sem isto as barras re-animariam a cada refresh de
  // 60s enquanto o popup fica aberto, o que é ruído, não sinal.
  property bool barsAnimating: false
  // Incrementado a cada 30s só para forçar QML a reavaliar bindings de
  // texto que dependem do relógio ("há Xm", countdown) — QML não reavalia
  // sozinho por passagem de tempo, só por mudança de dependência. Uso:
  // `text: (root.clockTick, root.fmtAgo(...))`.
  property int clockTick: 0

  readonly property color fg: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(fg, 1.45)
  readonly property string fontFamily: bar ? bar.fontFamily : "monospace"
  readonly property var providers: payload && Array.isArray(payload.providers) ? payload.providers : []
  // Clamp aos limites do schema do manifest (min 30 / max 3600) — o editor
  // do shell respeita o schema, mas shell.json editado à mão não.
  readonly property int refreshIntervalSec: root.clampRefreshInterval(Number(setting("refreshIntervalSec", 60)) || 60)
  readonly property var knownProviderIds: ["claude", "codex", "amp", "grok"]

  // Chamado pelo PopupCard no clique-fora (owner.close()). Sem isto o card
  // faz `open = false` imperativo, o que DESTRÓI o binding open:popupOpen —
  // o popup nunca mais abre. Resetar a fonte da verdade preserva o binding.
  function close() {
    popupOpen = false
    settingsMode = false
    settingsBusy = false
  }

  // PopupCard (xdg-popup) não tem focusTarget como KeyboardPanel; forçamos
  // o catcher quando o popup abre. Sem click no card o compositor pode ainda
  // não entregar teclas — mesmo padrão do shell quando o surface não é Exclusive.
  onPopupOpenChanged: {
    if (popupOpen) {
      // M1 é "abertura", não "toda atualização" — arma a janela de motion
      // e desarma sozinho depois (Timer abaixo), pra refresh de fundo não
      // re-disparar o preenchimento das barras.
      root.barsAnimating = root.menuAnimationsEnabled
      barsAnimationResetTimer.restart()
      Qt.callLater(function() {
        if (root.popupOpen && keyCatcher) keyCatcher.forceActiveFocus()
      })
    }
  }

  // 1500ms cobre o pior caso plausível (~4 providers x 3-4 linhas, stagger
  // 60ms por linha dentro de cada painel + 320ms de duração) com folga.
  Timer {
    id: barsAnimationResetTimer
    interval: 1500
    running: false
    repeat: false
    onTriggered: root.barsAnimating = false
  }

  function pathFromUrl(url) { return String(url).replace(/^file:\/\//, "") }
  readonly property string helperPath: pathFromUrl(Qt.resolvedUrl("scripts/agent-bar-open-terminal"))

  // Espelha severity_color_api (severity.rs): API conhecida vence o
  // threshold local; desconhecida/ausente cai em >=60 ok / 30-59 low /
  // 10-29 warn / <10 critical.
  function severityBucket(w) {
    if (!w) return "ok"
    var api = String(w.severity || "").toLowerCase()
    if (api === "normal" || api === "ok") return "ok"
    if (api === "warning" || api === "elevated" || api === "high") return "warn"
    if (api === "critical" || api === "exceeded" || api === "blocked") return "critical"
    var pct = Number(w.remaining)
    if (!isFinite(pct)) return "ok"
    if (pct < 10) return "critical"
    if (pct < 30) return "warn"
    if (pct < 60) return "low"
    return "ok"
  }

  function severityColor(bucket) {
    if (bucket === "critical") return urgent
    if (bucket === "warn") return Qt.tint(fg, Qt.rgba(urgent.r, urgent.g, urgent.b, 0.55))
    if (bucket === "low") return Qt.tint(fg, Qt.rgba(urgent.r, urgent.g, urgent.b, 0.30))
    return fg
  }

  function iconFile(id) {
    if (id === "claude") return "icons/claude-code-icon.png"
    if (id === "codex") return "icons/codex-icon.png"
    if (id === "amp") return "icons/amp-icon.svg"
    if (id === "grok") return "icons/grok-icon.svg"
    return ""
  }

  function providerLabel(id) {
    if (id === "claude") return "Claude"
    if (id === "codex") return "Codex"
    if (id === "amp") return "Amp"
    if (id === "grok") return "Grok"
    return String(id || "")
  }

  // ── Display value (remaining vs used) ────────────────────────────────
  // Espelha `to_window_display` (src/formatters/shared.rs): honra `used`
  // do provider quando `mode === "used"` e o campo existe; senão deriva de
  // `remaining`. Usado tanto pelo chip da barra quanto pelas linhas do popup
  // (spec: "displayMode used passa a valer nas linhas, não só no chip").
  function windowDisplayValue(w, mode) {
    if (!w) return NaN
    if (mode === "used") {
      var used = Number(w.used)
      if (isFinite(used)) return used
      var rem = Number(w.remaining)
      return isFinite(rem) ? 100 - rem : NaN
    }
    return Number(w.remaining)
  }

  function formatDisplayPct(v) {
    return isFinite(v) ? Math.round(v) + "%" : ""
  }

  function chipLabel(p) {
    if (!p.available) return "–"
    return root.formatDisplayPct(root.windowDisplayValue(p.primary, root.displayMode))
  }

  // Mesmo cálculo de chipLabel, mas para a prévia ao vivo do Settings — usa
  // o modo em RASCUNHO (draftSettings), não o modo salvo, para o usuário ver
  // o efeito antes de clicar Salvar.
  function previewChipLabel(p) {
    if (!p.available) return "–"
    var mode = root.draftSettings.displayMode === "used" ? "used" : "remaining"
    return root.formatDisplayPct(root.windowDisplayValue(p.primary, mode))
  }

  // ── Rótulo de janela — só a partir de windowKind (Plano 02), zero magic
  // number aqui (o antigo ±90/±1440 morreu junto com o backend). "other"
  // ainda carrega windowMinutes cru; usamos só para um rótulo por duração
  // real, nunca para reclassificar.
  function windowLabel(w) {
    var kind = w ? String(w.windowKind || "") : ""
    if (kind === "fiveHour") return "Sessão 5h"
    if (kind === "sevenDay") return "Semana"
    if (kind === "daily") return "Diário"
    if (kind === "context") return "Contexto"
    return root.otherWindowLabel(w)
  }

  function otherWindowLabel(w) {
    var m = Number(w && w.windowMinutes)
    if (!isFinite(m) || m <= 0) return "Janela"
    if (m % 1440 === 0) return "Janela de " + (m / 1440) + "d"
    var h = Math.floor(m / 60)
    var mm = m % 60
    return mm === 0 ? ("Janela de " + h + "h") : ("Janela de " + h + "h" + mm + "m")
  }

  // ── Dedup display-level — espelha `is_duplicate_window`
  // (src/formatters/shared.rs, Plano 02): mesma (windowKind, resetsAt,
  // remaining arredondado) = mesma janela. O JSON continua emitindo
  // primary/secondary/models sem cortes; o corte é só aqui, na tela.
  function windowKey(w) {
    if (!w) return null
    var kind = String(w.windowKind || "other")
    var resets = w.resetsAt || ""
    var rem = isFinite(Number(w.remaining)) ? Math.round(Number(w.remaining)) : "?"
    return kind + "|" + resets + "|" + rem
  }

  function isDuplicateWindow(a, b) {
    if (!a || !b) return false
    return root.windowKey(a) === root.windowKey(b)
  }

  // primary/secondary sempre entram (rotulados por windowKind); um model só
  // entra quando NÃO duplica uma janela já incluída — mata o triplo "Weekly"
  // do Codex (primary e secondary na mesma janela) sem esconder um model que
  // genuinamente diverge (ex.: "Fable" com remaining diferente da agregada).
  function providerRows(p) {
    var rows = []
    function addRow(name, w, isSub) {
      if (!w) return
      for (var i = 0; i < rows.length; i++) {
        if (root.isDuplicateWindow(rows[i].window, w)) return
      }
      rows.push({ name: name, window: w, isSub: !!isSub })
    }
    addRow(root.windowLabel(p.primary), p.primary, false)
    addRow(root.windowLabel(p.secondary), p.secondary, false)
    var models = p.models || {}
    var keys = Object.keys(models)
    for (var j = 0; j < keys.length; j++) {
      addRow(keys[j], models[keys[j]], true)
    }
    return rows
  }

  // ── `extra` na tela — shapes documentados em docs/json-output.md /
  // src/providers/types.rs (AmpQuotaExtra.meta, GrokQuotaExtra,
  // ClaudeQuotaExtra.extraUsage). `extra` é unstable por contrato: qualquer
  // chave ausente vira linha omitida, nunca um valor inventado.
  function extraInfoRows(p) {
    var rows = []
    var extra = p.extra
    if (!extra) return rows
    if (p.provider === "amp") {
      var bal = extra.meta && extra.meta.creditsBalance
      if (bal) {
        var rep = (extra.meta.creditsReplenish === "auto") ? " · replenish automático" : ""
        rows.push({ label: "Créditos", info: "<b>" + bal + "</b>" + rep })
      }
    } else if (p.provider === "grok") {
      var sessions = Number(extra.sessionsToday)
      var turns = Number(extra.turnsToday)
      if (isFinite(sessions) || isFinite(turns)) {
        var parts = []
        if (isFinite(sessions)) parts.push("<b>" + sessions + " sessões</b>")
        if (isFinite(turns)) parts.push("<b>" + turns + " turnos</b>")
        if (extra.recentModel) parts.push(String(extra.recentModel))
        rows.push({ label: "Hoje", info: parts.join(" · ") })
      }
    } else if (p.provider === "claude") {
      var eu = extra.extraUsage
      if (eu && eu.enabled) {
        var used = Number(eu.used)
        var limit = Number(eu.limit)
        var info = (isFinite(limit) && limit > 0)
          ? "<b>$" + used.toFixed(2) + "</b> de $" + limit.toFixed(2)
          : "<b>$" + used.toFixed(2) + "</b> gasto"
        rows.push({ label: "Extra usage", info: info })
      }
    }
    return rows
  }

  // ── Relógio local (JS Date pega o fuso do SO automaticamente — sem
  // equivalente a `Clock.local_offset` do Rust necessário aqui). ─────────
  readonly property var ptWeekdays: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"]

  function clampRefreshInterval(sec) {
    var n = Math.round(Number(sec))
    if (!isFinite(n)) n = 60
    return Math.min(3600, Math.max(30, n))
  }

  function fmtAgo(iso, now) {
    if (!iso) return ""
    var d = new Date(iso)
    if (isNaN(d.getTime())) return ""
    var diffMs = now.getTime() - d.getTime()
    if (diffMs < 60000) return "agora mesmo"
    var mins = Math.floor(diffMs / 60000)
    if (mins < 60) return "há " + mins + "m"
    return "há " + Math.floor(mins / 60) + "h"
  }

  function fmtDuration(ms) {
    var totalMin = Math.max(0, Math.floor(ms / 60000))
    var days = Math.floor(totalMin / 1440)
    var hours = Math.floor((totalMin % 1440) / 60)
    var mins = totalMin % 60
    if (days > 0) return days + "d " + hours + "h"
    var mm = mins < 10 ? "0" + mins : String(mins)
    return hours + "h " + mm + "m"
  }

  function fmtLocalClock(d, now) {
    var hh = String(d.getHours()).padStart(2, "0")
    var mm = String(d.getMinutes()).padStart(2, "0")
    var sameDay = d.getFullYear() === now.getFullYear()
      && d.getMonth() === now.getMonth()
      && d.getDate() === now.getDate()
    if (sameDay) return hh + ":" + mm
    return root.ptWeekdays[d.getDay()] + " " + hh + ":" + mm
  }

  function fmtTokCount(n) {
    var v = Number(n)
    if (!isFinite(v)) return "?"
    if (v >= 1000) return (v / 1000).toFixed(1) + "k"
    return String(Math.round(v))
  }

  // Grok "context" não tem resetsAt (não é baseado em tempo) — a coluna de
  // ETA vira a razão de tokens usados/janela (spec: "253.9k/500k tok").
  function contextRatioLabel(extra) {
    if (!extra) return ""
    var used = Number(extra.contextTokensUsed)
    var total = Number(extra.contextWindowTokens)
    if (!isFinite(used) || !isFinite(total) || total <= 0) return ""
    return root.fmtTokCount(used) + "/" + root.fmtTokCount(total) + " tok"
  }

  // Coluna de reset/countdown da grade. "Full" quando remaining==100 (só
  // pra manter paridade com `format_eta` do Rust); "?" sem resetsAt; senão
  // "{duração} · {HH:MM ou seg HH:MM}" — dia da semana só aparece quando o
  // reset cai num dia diferente de `now` (local).
  function fmtEta(w, extra, now) {
    if (!w) return ""
    var kind = String(w.windowKind || "")
    if (kind === "context") return root.contextRatioLabel(extra)
    var rem = Number(w.remaining)
    if (isFinite(rem) && rem >= 100) return "Full"
    var iso = w.resetsAt
    if (!iso) return "?"
    var d = new Date(iso)
    if (isNaN(d.getTime())) return "?"
    var diffMs = d.getTime() - now.getTime()
    var dur = diffMs > 0 ? root.fmtDuration(diffMs) : "0h 00m"
    return dur + " · " + root.fmtLocalClock(d, now)
  }

  function applyPayload(raw) {
    try {
      var data = JSON.parse(String(raw || ""))
      if (!data || data.schemaVersion !== 1 || !Array.isArray(data.providers))
        throw new Error("unexpected envelope")
      root.payload = data
      root.stale = false
    } catch (e) {
      root.stale = true // mantém o último payload bom
    }
  }

  function refresh(force) {
    if (fetchProc.running) return
    root.forceNext = !!force
    fetchProc.running = true
  }

  function openTui() {
    // Util.shellQuote, não bar.shellQuote: o README do bar documenta
    // bar.shellQuote(), mas o Bar do 4.0.0.alpha não expõe a função —
    // widgets first-party usam Util (Workspaces.qml faz igual).
    if (bar) bar.run(Util.shellQuote(root.helperPath) + " agent-bar menu")
  }

  function applyConfigView(data) {
    if (!data || data.schemaVersion !== 1) return false
    enabledIds = Array.isArray(data.providerOrder) ? data.providerOrder.slice()
              : (Array.isArray(data.providers) ? data.providers.slice() : [])
    displayMode = data.displayMode === "used" ? "used" : "remaining"
    // Ausente/omitido no payload = true — nunca desliga motion à toa num
    // payload de config show mais velho que o campo.
    menuAnimationsEnabled = data.menuAnimations !== false
    return true
  }

  function onConfigShowFinished(text) {
    try {
      var data = JSON.parse(String(text || ""))
      if (!applyConfigView(data)) throw new Error("bad config")
      draftSettings = {
        providers: (data.providers || []).slice(),
        providerOrder: (data.providerOrder || data.providers || []).slice(),
        displayMode: displayMode,
        notifyEnabled: !!(data.notify && data.notify.enabled),
        refreshIntervalSec: root.clampRefreshInterval(Number(setting("refreshIntervalSec", 60)) || 60)
      }
      settingsStatusText = ""
    } catch (e) {
      settingsStatusText = "failed to load settings"
    }
  }

  function openSettings() {
    settingsMode = true
    popupOpen = true
    settingsStatusText = ""
    if (!configShowProc.running) configShowProc.running = true
  }

  function showUsage() {
    settingsMode = false
    settingsStatusText = ""
  }

  function visibleProviders() {
    var all = root.providers
    if (!enabledIds || !enabledIds.length) return all
    var out = []
    for (var i = 0; i < enabledIds.length; i++) {
      var id = enabledIds[i]
      for (var j = 0; j < all.length; j++) {
        if (all[j] && all[j].provider === id) { out.push(all[j]); break }
      }
    }
    return out
  }

  // apply via tempfile: Process.write existe, mas fechar stdin (EOF) para
  // `config apply --json -` não é API estável no Quickshell do Omarchy
  // 4.0.0.alpha — mktemp + --file evita o problema.
  function applyShellScript(blob) {
    return "f=$(mktemp) && printf '%s' " + Util.shellQuote(String(blob || ""))
      + " >\"$f\" && agent-bar config apply --file \"$f\"; e=$?; rm -f \"$f\"; exit $e"
  }

  function runConfigApply(blob) {
    if (configApplyProc.running) return
    configApplyProc._haveStdout = false
    configApplyProc._haveExit = false
    configApplyProc._stdout = ""
    configApplyProc._exitCode = 0
    configApplyProc.command = ["bash", "-lc", root.applyShellScript(blob)]
    configApplyProc.running = true
  }

  function saveSettings() {
    if (settingsBusy) return
    var prov = draftSettings.providers || []
    if (!prov.length) {
      settingsStatusText = "Keep at least one provider"
      return
    }
    settingsBusy = true
    settingsStatusText = "Saving…"
    var blob = JSON.stringify({
      schemaVersion: 1,
      providers: prov,
      providerOrder: draftSettings.providerOrder || prov,
      displayMode: draftSettings.displayMode === "used" ? "used" : "remaining",
      notify: { enabled: !!draftSettings.notifyEnabled }
    })
    root.runConfigApply(blob)
  }

  function onConfigApplyFinished(text, exitCode) {
    settingsBusy = false
    if (exitCode !== 0) {
      settingsStatusText = "apply failed"
      return
    }
    try {
      var data = JSON.parse(String(text || ""))
      if (!applyConfigView(data)) throw new Error("bad apply result")
    } catch (e) {
      settingsStatusText = "apply failed"
      return
    }
    // interval → shell.json (só se apply OK)
    var interval = root.clampRefreshInterval(Number(draftSettings.refreshIntervalSec) || 60)
    var intervalPersisted = false
    if (bar && bar.shell && typeof bar.shell.updateEntryInline === "function") {
      try {
        var next = Object.assign({}, root.settings || {}, { refreshIntervalSec: interval })
        bar.shell.updateEntryInline(root.moduleName, next)
        intervalPersisted = true
      } catch (e2) {
        intervalPersisted = false
      }
    }
    settingsStatusText = intervalPersisted
      ? "Saved"
      : "settings saved; interval not persisted"
    root.refresh(true)
  }

  function setProviderEnabled(id, on) {
    var p = (draftSettings.providers || []).slice()
    var idx = p.indexOf(id)
    if (on && idx < 0) p.push(id)
    if (!on) {
      if (p.length <= 1 && idx >= 0) {
        settingsStatusText = "Mantenha ao menos 1 provider ativo"
        return
      }
      if (idx >= 0) p.splice(idx, 1)
    }
    var order = (draftSettings.providerOrder || []).filter(function(x) { return p.indexOf(x) >= 0 })
    p.forEach(function(x) { if (order.indexOf(x) < 0) order.push(x) })
    draftSettings = Object.assign({}, draftSettings, { providers: p, providerOrder: order })
  }

  function moveProvider(id, dir) {
    var p = (draftSettings.providers || []).slice()
    var idx = p.indexOf(id)
    if (idx < 0) return
    var j = idx + dir
    if (j < 0 || j >= p.length) return
    var tmp = p[idx]
    p[idx] = p[j]
    p[j] = tmp
    draftSettings = Object.assign({}, draftSettings, { providers: p, providerOrder: p.slice() })
  }

  function setDisplayMode(mode) {
    var m = mode === "used" ? "used" : "remaining"
    draftSettings = Object.assign({}, draftSettings, { displayMode: m })
  }

  function setNotifyEnabled(on) {
    draftSettings = Object.assign({}, draftSettings, { notifyEnabled: !!on })
  }

  function setRefreshInterval(sec) {
    draftSettings = Object.assign({}, draftSettings, { refreshIntervalSec: root.clampRefreshInterval(sec) })
  }

  // enabled first (draft order), then disabled known ids
  function settingsProviderRows() {
    var known = root.knownProviderIds
    var enabled = draftSettings.providers || []
    var rows = []
    for (var i = 0; i < enabled.length; i++) {
      if (known.indexOf(enabled[i]) >= 0)
        rows.push({ id: enabled[i], on: true, index: i, count: enabled.length })
    }
    for (var j = 0; j < known.length; j++) {
      if (enabled.indexOf(known[j]) < 0)
        rows.push({ id: known[j], on: false, index: -1, count: enabled.length })
    }
    return rows
  }

  implicitWidth: root.vertical ? root.barSize : chips.implicitWidth + 12
  implicitHeight: root.vertical ? chips.implicitHeight + 12 : root.barSize
  opacity: root.stale ? 0.55 : 1.0

  Component.onCompleted: {
    if (!configShowProc.running) configShowProc.running = true
  }

  Process {
    id: fetchProc
    command: ["bash", "-lc", root.forceNext ? "agent-bar --format json --refresh" : "agent-bar --format json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.applyPayload(text)
        root.forceNext = false
      }
    }
  }

  Process {
    id: configShowProc
    command: ["bash", "-lc", "agent-bar config show --format json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.onConfigShowFinished(text)
    }
  }

  Process {
    id: configApplyProc
    // Barrier: onExited can race ahead of StdioCollector flush.
    // Wait for both streamFinished and exited before parsing stdout.
    property string _stdout: ""
    property int _exitCode: 0
    property bool _haveStdout: false
    property bool _haveExit: false
    command: ["bash", "-lc", "true"]
    stdout: StdioCollector {
      id: applyOut
      waitForEnd: true
      onStreamFinished: {
        configApplyProc._stdout = text
        configApplyProc._haveStdout = true
        configApplyProc._tryFinishApply()
      }
    }
    onExited: function(exitCode, exitStatus) {
      configApplyProc._exitCode = exitCode
      configApplyProc._haveExit = true
      configApplyProc._tryFinishApply()
    }
    function _tryFinishApply() {
      if (!_haveStdout || !_haveExit) return
      var text = _stdout
      var code = _exitCode
      _haveStdout = false
      _haveExit = false
      _stdout = ""
      root.onConfigApplyFinished(text, code)
    }
  }

  Timer {
    interval: root.refreshIntervalSec * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh(false)
  }

  // Só força reavaliação de texto ("há Xm", countdown) — nunca dispara
  // fetch nem I/O.
  Timer {
    interval: 30000
    running: true
    repeat: true
    onTriggered: root.clockTick++
  }

  Grid {
    id: chips
    anchors.centerIn: parent
    columns: root.vertical ? 1 : Math.max(1, root.visibleProviders().length)
    columnSpacing: 10
    rowSpacing: 6

    Repeater {
      model: root.visibleProviders()

      Item {
        id: chip
        required property var modelData
        readonly property string bucket: root.severityBucket(modelData.primary)
        readonly property string label: root.chipLabel(modelData)

        width: chipRow.implicitWidth
        height: root.vertical ? root.barSize - 6 : root.barSize

        Row {
          id: chipRow
          anchors.centerIn: parent
          spacing: 4
          opacity: chip.modelData.staleReason ? 0.55 : 1.0

          Image {
            source: root.iconFile(chip.modelData.provider) ? Qt.resolvedUrl(root.iconFile(chip.modelData.provider)) : ""
            visible: source !== ""
            width: 13
            height: 13
            sourceSize.width: 13
            sourceSize.height: 13
            fillMode: Image.PreserveAspectFit
            anchors.verticalCenter: parent.verticalCenter
            opacity: chip.modelData.available ? 1.0 : 0.45
          }

          Text {
            visible: !root.vertical && chip.label !== ""
            text: chip.label
            color: root.severityColor(chip.bucket)
            font.family: root.fontFamily
            font.pixelSize: 12
            anchors.verticalCenter: parent.verticalCenter
          }
        }

        MouseArea {
          anchors.fill: parent
          acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
          hoverEnabled: true
          onEntered: if (root.bar) root.bar.showTooltip(chip, chip.modelData.displayName + (chip.label ? " · " + chip.label : "") + (chip.modelData.available ? "" : " · " + String(chip.modelData.error || "unavailable")))
          onExited: if (root.bar) root.bar.hideTooltip(chip)
          onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) root.openSettings()
            else if (mouse.button === Qt.MiddleButton) root.refresh(true)
            else {
              // left: usage; toggle open only if already usage (or closed).
              // From settings with popup open → switch to usage, keep open.
              if (root.settingsMode && root.popupOpen) {
                root.showUsage()
              } else {
                root.showUsage()
                root.popupOpen = !root.popupOpen
              }
            }
          }
        }
      }
    }

    Text {
      visible: root.visibleProviders().length === 0
      text: "agent-bar"
      color: root.fg
      font.family: root.fontFamily
      font.pixelSize: 12
    }
  }

  PopupCard {
    id: popup
    anchorItem: root
    owner: root
    bar: root.bar
    open: root.popupOpen
    contentWidth: Style.space(540)
    contentHeight: Math.min(popupCol.implicitHeight + Style.space(20), Style.space(560))

    // PanelKeyCatcher (qs.Ui): esc → close. Salvar só pelo botão (atalho s
    // removido — decisão travada do mockup settings-v2).
    // PopupCard não expõe focusTarget; ver onPopupOpenChanged no root.
    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      blocked: false

      onCloseRequested: root.close()

    Flickable {
      anchors.fill: parent
      contentHeight: popupCol.implicitHeight
      clip: true

      ColumnLayout {
        id: popupCol
        width: parent.width
        spacing: 10

        // ── Titlebar de uso (B1: título à esquerda, ações à direita) ───
        RowLayout {
          visible: !root.settingsMode
          Layout.fillWidth: true
          spacing: 6

          Text {
            text: "agent-bar"
            color: root.fg
            font.family: root.fontFamily
            font.pixelSize: 13
            font.bold: true
          }
          Text {
            text: (root.clockTick, root.payload ? "· " + root.fmtAgo(root.payload.fetchedAt, new Date()) : "")
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: 11
          }

          Item { Layout.fillWidth: true }

          // Glifos Nerd Font / Font Awesome (PUA) — o bar.fontFamily resolve
          // pra JetBrainsMono Nerd Font no Omarchy; Unicode "bonito" (↻ ⚙︎ ❯)
          // cai em glyphs quebrados/incompletos nessa família monoespaçada.
          IconButton {
            glyph: "\uf021" // fa-refresh
            tooltip: "Atualizar"
            spinning: fetchProc.running
            onClicked: root.refresh(true)
          }
          IconButton {
            glyph: "\uf013" // fa-cog
            tooltip: "Settings"
            onClicked: root.openSettings()
          }
          IconButton {
            glyph: "\uf054" // fa-chevron-right
            tooltip: "Abrir TUI"
            onClicked: root.openTui()
          }
        }

        // ── Titlebar de settings ────────────────────────────────────────
        RowLayout {
          visible: root.settingsMode
          Layout.fillWidth: true
          spacing: 8

          Text {
            text: "agent-bar"
            color: root.fg
            font.family: root.fontFamily
            font.pixelSize: 13
            font.bold: true
          }
          Text {
            text: "· Settings"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: 13
          }

          Item { Layout.fillWidth: true }

          Button {
            text: "← Uso"
            foreground: root.fg
            fontFamily: root.fontFamily
            fontSize: 10
            horizontalPadding: 8
            verticalPadding: 4
            onClicked: root.showUsage()
          }
          Button {
            text: root.settingsBusy ? "Salvando…" : "Salvar"
            foreground: root.fg
            accent: Color.accent
            fontFamily: root.fontFamily
            fontSize: 10
            horizontalPadding: 8
            verticalPadding: 4
            active: true
            enabled: !root.settingsBusy
            onClicked: root.saveSettings()
          }
        }

        // ── Usage: um painel elevado por provider (Estilo 2 — Painéis) ──
        Repeater {
          model: root.settingsMode ? [] : root.visibleProviders()

          ColumnLayout {
            id: panelWrap
            required property var modelData
            Layout.fillWidth: true
            spacing: 0

            Rectangle {
              Layout.fillWidth: true
              implicitHeight: panelContent.implicitHeight + 20
              radius: 9
              // Tint semi-transparente de root.fg — os hex do mockup são só
              // do protótipo HTML; a cor real do tema do shell entra via fg.
              color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.05)
              border.width: 1
              border.color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.14)
              opacity: panelWrap.modelData.staleReason ? 0.55 : 1.0

              ColumnLayout {
                id: panelContent
                anchors.fill: parent
                anchors.margins: 10
                spacing: 4

                RowLayout {
                  Layout.fillWidth: true
                  spacing: 6

                  Image {
                    source: root.iconFile(panelWrap.modelData.provider) ? Qt.resolvedUrl(root.iconFile(panelWrap.modelData.provider)) : ""
                    visible: source !== ""
                    width: 15; height: 15
                    sourceSize.width: 15; sourceSize.height: 15
                    fillMode: Image.PreserveAspectFit
                  }
                  Text {
                    text: panelWrap.modelData.displayName
                    color: root.fg
                    font.family: root.fontFamily
                    font.pixelSize: 13
                    font.bold: true
                  }
                  Text {
                    Layout.fillWidth: true
                    text: String(panelWrap.modelData.plan || panelWrap.modelData.account || "")
                    color: root.dim
                    font.family: root.fontFamily
                    font.pixelSize: 11
                    elide: Text.ElideRight
                  }
                  // Hero % — mesmo valor e severidade do chip da barra.
                  Text {
                    visible: panelWrap.modelData.available
                    text: root.chipLabel(panelWrap.modelData)
                    color: root.severityColor(root.severityBucket(panelWrap.modelData.primary))
                    font.family: root.fontFamily
                    font.pixelSize: 15
                    font.bold: true
                  }
                }

                Text {
                  visible: !panelWrap.modelData.available
                  text: String(panelWrap.modelData.error || "Unavailable")
                  color: Qt.darker(root.fg, 1.3)
                  font.family: root.fontFamily
                  font.pixelSize: 11
                  wrapMode: Text.WordWrap
                  Layout.fillWidth: true
                }

                Text {
                  visible: !!panelWrap.modelData.staleReason
                  text: "stale — " + String(panelWrap.modelData.staleReason || "")
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: 10
                  wrapMode: Text.WordWrap
                  Layout.fillWidth: true
                }

                Repeater {
                  model: panelWrap.modelData.available ? root.providerRows(panelWrap.modelData) : []

                  QuotaRow {
                    window: modelData.window
                    extra: panelWrap.modelData.extra
                    name: modelData.name
                    isSub: modelData.isSub
                    rowIndex: index
                  }
                }

                Repeater {
                  model: panelWrap.modelData.available ? root.extraInfoRows(panelWrap.modelData) : []

                  InfoRow {
                    label: modelData.label
                    info: modelData.info
                  }
                }
              }
            }
          }
        }

        // ── Settings body v2: 3 painéis (Providers / Exibição / Alertas &
        // atualização), prévia ao vivo da barra, sem rodapé de dicas ──────
        Rectangle {
          visible: root.settingsMode
          Layout.fillWidth: true
          implicitHeight: providersCol.implicitHeight + 20
          radius: 9
          color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.05)
          border.width: 1
          border.color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.14)

          ColumnLayout {
            id: providersCol
            anchors.fill: parent
            anchors.margins: 10
            spacing: 8

            PanelSectionHeader {
              Layout.fillWidth: true
              text: "Providers"
              foreground: root.fg
              fontFamily: root.fontFamily
              fontSize: 11
            }

            Repeater {
              model: root.settingsMode ? root.settingsProviderRows() : []

              ColumnLayout {
                id: provRow
                required property var modelData
                Layout.fillWidth: true
                spacing: 4

                RowLayout {
                  Layout.fillWidth: true
                  spacing: 8

                  Image {
                    source: root.iconFile(provRow.modelData.id) ? Qt.resolvedUrl(root.iconFile(provRow.modelData.id)) : ""
                    visible: source !== ""
                    width: 15; height: 15
                    sourceSize.width: 15; sourceSize.height: 15
                    fillMode: Image.PreserveAspectFit
                  }
                  Toggle {
                    Layout.fillWidth: true
                    label: root.providerLabel(provRow.modelData.id)
                    description: checked ? "Visível na barra" : "Oculto da barra"
                    checked: provRow.modelData.on
                    foreground: root.fg
                    accent: Color.accent
                    fontFamily: root.fontFamily
                    onClicked: root.setProviderEnabled(provRow.modelData.id, !checked)
                  }
                }

                RowLayout {
                  visible: provRow.modelData.on
                  Layout.fillWidth: true
                  spacing: 6

                  Item { Layout.fillWidth: true }

                  IconButton {
                    glyph: "↑"
                    tooltip: "Mover para cima"
                    enabled: provRow.modelData.index > 0
                    onClicked: root.moveProvider(provRow.modelData.id, -1)
                  }
                  IconButton {
                    glyph: "↓"
                    tooltip: "Mover para baixo"
                    enabled: provRow.modelData.index >= 0 && provRow.modelData.index < provRow.modelData.count - 1
                    onClicked: root.moveProvider(provRow.modelData.id, 1)
                  }
                }
              }
            }
          }
        }

        Rectangle {
          visible: root.settingsMode
          Layout.fillWidth: true
          implicitHeight: exibicaoCol.implicitHeight + 20
          radius: 9
          color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.05)
          border.width: 1
          border.color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.14)

          ColumnLayout {
            id: exibicaoCol
            anchors.fill: parent
            anchors.margins: 10
            spacing: 8

            PanelSectionHeader {
              Layout.fillWidth: true
              text: "Exibição"
              foreground: root.fg
              fontFamily: root.fontFamily
              fontSize: 11
            }

            RowLayout {
              Layout.fillWidth: true
              spacing: 6

              Text {
                text: "Número do chip"
                color: root.fg
                font.family: root.fontFamily
                font.pixelSize: 12
                Layout.fillWidth: true
              }
              Button {
                text: "Restante"
                foreground: root.fg
                accent: Color.accent
                fontFamily: root.fontFamily
                fontSize: 11
                horizontalPadding: 10
                verticalPadding: 5
                active: (root.draftSettings.displayMode || "remaining") !== "used"
                onClicked: root.setDisplayMode("remaining")
              }
              Button {
                text: "Usado"
                foreground: root.fg
                accent: Color.accent
                fontFamily: root.fontFamily
                fontSize: 11
                horizontalPadding: 10
                verticalPadding: 5
                active: root.draftSettings.displayMode === "used"
                onClicked: root.setDisplayMode("used")
              }
            }

            // Prévia ao vivo: usa o modo em RASCUNHO contra os dados reais
            // já carregados — o usuário vê o efeito antes de clicar Salvar.
            RowLayout {
              Layout.fillWidth: true
              spacing: 14

              Text {
                text: "Prévia"
                color: Qt.darker(root.fg, 1.6)
                font.family: root.fontFamily
                font.pixelSize: 10
              }

              Repeater {
                model: root.settingsMode ? root.visibleProviders() : []

                RowLayout {
                  required property var modelData
                  spacing: 5

                  Image {
                    source: root.iconFile(modelData.provider) ? Qt.resolvedUrl(root.iconFile(modelData.provider)) : ""
                    visible: source !== ""
                    width: 13; height: 13
                    sourceSize.width: 13; sourceSize.height: 13
                    fillMode: Image.PreserveAspectFit
                  }
                  Text {
                    text: root.previewChipLabel(modelData)
                    color: root.severityColor(root.severityBucket(modelData.primary))
                    font.family: root.fontFamily
                    font.pixelSize: 12
                  }
                }
              }
            }
          }
        }

        Rectangle {
          visible: root.settingsMode
          Layout.fillWidth: true
          implicitHeight: alertasCol.implicitHeight + 20
          radius: 9
          color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.05)
          border.width: 1
          border.color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.14)

          ColumnLayout {
            id: alertasCol
            anchors.fill: parent
            anchors.margins: 10
            spacing: 8

            PanelSectionHeader {
              Layout.fillWidth: true
              text: "Alertas & atualização"
              foreground: root.fg
              fontFamily: root.fontFamily
              fontSize: 11
            }

            Toggle {
              Layout.fillWidth: true
              label: "Notificações desktop"
              description: checked ? "notify-send nos limites de quota" : "Notificações desligadas"
              checked: !!root.draftSettings.notifyEnabled
              foreground: root.fg
              accent: Color.accent
              fontFamily: root.fontFamily
              onClicked: root.setNotifyEnabled(!checked)
            }

            RowLayout {
              Layout.fillWidth: true
              spacing: 8

              ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                Text {
                  text: "Intervalo"
                  color: root.fg
                  font.family: root.fontFamily
                  font.pixelSize: 12
                }
                Text {
                  text: "segundos entre atualizações"
                  color: Qt.darker(root.fg, 1.6)
                  font.family: root.fontFamily
                  font.pixelSize: 11
                }
              }

              IconButton {
                glyph: "−"
                tooltip: "Diminuir 30s"
                enabled: root.draftSettings.refreshIntervalSec > 30
                onClicked: root.setRefreshInterval(root.draftSettings.refreshIntervalSec - 30)
              }
              Text {
                text: root.draftSettings.refreshIntervalSec + "s"
                color: root.fg
                font.family: root.fontFamily
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                Layout.preferredWidth: 46
              }
              IconButton {
                glyph: "+"
                tooltip: "Aumentar 30s"
                enabled: root.draftSettings.refreshIntervalSec < 3600
                onClicked: root.setRefreshInterval(root.draftSettings.refreshIntervalSec + 30)
              }
            }
          }
        }

        Text {
          visible: root.settingsMode && root.settingsStatusText !== ""
          Layout.fillWidth: true
          text: root.settingsStatusText
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: 10
          horizontalAlignment: Text.AlignHCenter
          wrapMode: Text.WordWrap
        }
      }
    }
    } // PanelKeyCatcher
  }

  // Botão quadrado só-ícone. Preferir glifos Nerd Font/Font Awesome no PUA
  // (ex. "\uf021") — o bar.fontFamily no Omarchy é monoespaçado Nerd e
  // renderiza esses com fidelidade; Unicode "decorativo" (↻ ⚙︎ ❯) quebra.
  // Reusado para ↑/↓/−/+ (ASCII, estáveis em mono).
  //
  // Centralização: Text preenche o botão + AlignHCenter/VCenter (padrão do
  // PanelActionButton do Omarchy). NÃO usar TextMetrics.tightBoundingRect —
  // com FA no PUA o tight rect às vezes vem vazio/errado e o glifo some
  // pra fora do botão.
  component IconButton: Item {
    id: btn
    property string glyph: ""
    property string tooltip: ""
    property bool spinning: false
    signal clicked()

    readonly property int side: 28
    // 12px cabe limpo no 28×28 com padding visual; Style.font.icon (≈14)
    // fica apertado e parece desalinhado.
    readonly property int glyphPx: 12

    implicitWidth: side
    implicitHeight: side
    opacity: btn.enabled ? 1.0 : 0.4

    Rectangle {
      anchors.fill: parent
      radius: 6
      color: mouse.containsMouse && btn.enabled
        ? Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.14)
        : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.08)
      border.width: 1
      border.color: mouse.containsMouse && btn.enabled
        ? Color.accent
        : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.20)

      Behavior on color { enabled: root.menuAnimationsEnabled; ColorAnimation { duration: 160 } }
      Behavior on border.color { enabled: root.menuAnimationsEnabled; ColorAnimation { duration: 160 } }
    }

    Text {
      id: glyphText
      anchors.fill: parent
      text: btn.glyph
      color: mouse.containsMouse && btn.enabled ? Color.accent : root.fg
      font.family: root.fontFamily
      font.pixelSize: btn.glyphPx
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      renderType: Text.NativeRendering
      // Origem no centro do botão — spin do refresh não “órbita” o glifo.
      transformOrigin: Item.Center

      RotationAnimation {
        target: glyphText
        property: "rotation"
        running: btn.spinning && root.menuAnimationsEnabled
        from: 0; to: 360
        duration: 900
        loops: Animation.Infinite
        onRunningChanged: if (!running) glyphText.rotation = 0
      }
    }

    MouseArea {
      id: mouse
      anchors.fill: parent
      enabled: btn.enabled
      hoverEnabled: true
      cursorShape: btn.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onEntered: if (root.bar && btn.tooltip) root.bar.showTooltip(btn, btn.tooltip)
      onExited: if (root.bar) root.bar.hideTooltip(btn)
      onClicked: btn.clicked()
    }
  }

  // Colunas fixas (spec): rótulo 82 · barra flex · % 44 · reset/eta 132.
  // Toda barra idêntica, % e reset sempre na mesma coluna independente do
  // provider — é o que mata a assimetria da versão antiga.
  component QuotaRow: RowLayout {
    id: row
    property var window: null
    property var extra: null
    property string name: ""
    property bool isSub: false
    property int rowIndex: 0
    visible: !!window
    Layout.fillWidth: true
    spacing: 6

    Text {
      text: row.name
      color: row.isSub ? Qt.darker(root.fg, 1.6) : Qt.darker(root.fg, 1.3)
      font.family: root.fontFamily
      font.pixelSize: 11
      leftPadding: row.isSub ? 12 : 0
      Layout.preferredWidth: 82
      elide: Text.ElideRight
    }

    Item {
      Layout.fillWidth: true
      height: 6

      Rectangle {
        anchors.fill: parent
        radius: 3
        color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.18)
      }
      Rectangle {
        readonly property real used: row.window && isFinite(Number(row.window.remaining)) ? (100 - Math.max(0, Math.min(100, Number(row.window.remaining)))) / 100 : 0
        width: parent.width * used
        height: parent.height
        radius: 3
        color: root.severityColor(root.severityBucket(row.window))

        // M1: só anima durante a janela de abertura do popup (barsAnimating);
        // fora dela o valor só salta, sem transição — refresh de fundo não
        // deve re-disparar o preenchimento. Gotcha: Behavior on width não
        // anima a atribuição inicial na construção do item — se na prova
        // ao vivo as barras não animarem no 1º open, fallback é width:0 +
        // Component.onCompleted (Task 6 do plano).
        Behavior on width {
          enabled: root.barsAnimating && root.menuAnimationsEnabled
          SequentialAnimation {
            PauseAnimation { duration: Math.min(row.rowIndex, 8) * 60 }
            NumberAnimation { duration: 320; easing.type: Easing.OutCubic }
          }
        }
      }
    }

    Text {
      text: root.formatDisplayPct(root.windowDisplayValue(row.window, root.displayMode))
      color: Qt.darker(root.fg, 1.3)
      font.family: root.fontFamily
      font.pixelSize: 11
      Layout.preferredWidth: 44
      horizontalAlignment: Text.AlignRight
    }

    Text {
      text: (root.clockTick, root.fmtEta(row.window, row.extra, new Date()))
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: 11
      Layout.preferredWidth: 132
      horizontalAlignment: Text.AlignRight
      elide: Text.ElideRight
    }
  }

  // Linha de `extra` (Créditos/Hoje/Extra usage) — mesma coluna de rótulo
  // (82) do QuotaRow; o valor ocupa o resto da largura (aproxima o
  // `grid-column: 2/5` do mockup sem depender da grade CSS que QML não tem).
  component InfoRow: RowLayout {
    id: infoRow
    property string label: ""
    property string info: ""
    visible: info !== ""
    Layout.fillWidth: true
    spacing: 6

    Text {
      text: infoRow.label
      color: Qt.darker(root.fg, 1.3)
      font.family: root.fontFamily
      font.pixelSize: 11
      Layout.preferredWidth: 82
      elide: Text.ElideRight
    }
    Text {
      text: infoRow.info
      textFormat: Text.RichText
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: 11
      Layout.fillWidth: true
      wrapMode: Text.WordWrap
    }
  }
}

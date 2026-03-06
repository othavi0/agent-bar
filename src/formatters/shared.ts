import type {
  ModelWindows,
  ProviderQuota,
  QuotaWindow,
} from "../providers/types";

export type WindowKind = "fiveHour" | "sevenDay" | "other";

export function formatPercent(val: number | null): string {
  return val === null ? "?%" : `${Math.round(val)}%`;
}

export function formatEta(
  iso: string | null,
  remaining: number | null,
): string {
  if (remaining === 100) return "Full";
  if (!iso) return "?";
  const diff = new Date(iso).getTime() - Date.now();
  if (diff < 0) return "0m";
  const d = Math.floor(diff / 86400000);
  const h = Math.floor((diff % 86400000) / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  return d > 0
    ? `${d}d ${h.toString().padStart(2, "0")}h`
    : `${h}h ${m.toString().padStart(2, "0")}m`;
}

export function formatResetTime(
  iso: string | null,
  remaining: number | null,
): string {
  if (remaining === 100) return "";
  if (!iso) return "(??:??)";
  const d = new Date(iso);
  return `(${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")})`;
}

export function classifyWindow(
  minutes: number | null | undefined,
): WindowKind {
  if (!minutes || minutes <= 0) return "other";
  if (Math.abs(minutes - 300) <= 90) return "fiveHour";
  if (Math.abs(minutes - 10080) <= 1440) return "sevenDay";
  return "other";
}

export function normalizePlanLabel(p: ProviderQuota): string {
  if (p.plan?.trim()) return p.plan;
  const raw = p.planType?.trim();
  if (!raw) return "Unknown";

  const key = raw.toLowerCase();
  const map: Record<string, string> = {
    free: "Free",
    go: "Go",
    plus: "Plus",
    pro: "Pro",
    business: "Business",
    team: "Business",
    enterprise: "Enterprise",
    edu: "Edu",
    education: "Edu",
    apikey: "API Key",
    api_key: "API Key",
  };
  return map[key] ?? raw;
}

export interface CodexModelEntry {
  name: string;
  windows: ModelWindows;
  severity: number;
}

export function codexModelsFromQuota(p: ProviderQuota): CodexModelEntry[] {
  const models: Record<string, ModelWindows> = {};

  if (p.modelsDetailed) {
    for (const [name, windows] of Object.entries(p.modelsDetailed)) {
      models[name] = windows;
    }
  }

  if (p.models) {
    for (const [name, window] of Object.entries(p.models)) {
      if (!models[name]) models[name] = {};
      const kind = classifyWindow(window.windowMinutes);
      if (kind === "fiveHour" && !models[name].fiveHour)
        models[name].fiveHour = window;
      else if (kind === "sevenDay" && !models[name].sevenDay)
        models[name].sevenDay = window;
      else {
        if (!models[name].other) models[name].other = [];
        models[name].other!.push(window);
      }
    }
  }

  if (Object.keys(models).length === 0 && (p.primary || p.secondary)) {
    const fallback: ModelWindows = {};
    for (const window of [p.primary, p.secondary]) {
      if (!window) continue;
      const kind = classifyWindow(window.windowMinutes);
      if (kind === "fiveHour" && !fallback.fiveHour) fallback.fiveHour = window;
      else if (kind === "sevenDay" && !fallback.sevenDay)
        fallback.sevenDay = window;
      else {
        if (!fallback.other) fallback.other = [];
        fallback.other.push(window);
      }
    }
    models["Codex"] = fallback;
  }

  return Object.entries(models)
    .map(([name, windows]) => {
      const values = [
        windows.fiveHour?.remaining,
        windows.sevenDay?.remaining,
        ...(windows.other?.map((w: QuotaWindow) => w.remaining) ?? []),
      ].filter((v): v is number => v !== undefined && v !== null);

      return {
        name,
        windows,
        severity: values.length > 0 ? Math.min(...values) : 101,
      };
    })
    .sort((a, b) => a.severity - b.severity || a.name.localeCompare(b.name));
}

export function applyCodexModelFilter(
  models: CodexModelEntry[],
  allowed?: string[],
): CodexModelEntry[] {
  if (!allowed || allowed.length === 0) return models;
  return models.filter((m) => allowed.includes(m.name));
}

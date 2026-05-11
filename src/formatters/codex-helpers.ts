import type { ModelWindows, ProviderQuota, QuotaWindow } from '../providers/types';
import { classifyWindow } from './shared';

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
      if (kind === 'fiveHour' && !models[name].fiveHour) models[name].fiveHour = window;
      else if (kind === 'sevenDay' && !models[name].sevenDay) models[name].sevenDay = window;
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
      if (kind === 'fiveHour' && !fallback.fiveHour) fallback.fiveHour = window;
      else if (kind === 'sevenDay' && !fallback.sevenDay) fallback.sevenDay = window;
      else {
        if (!fallback.other) fallback.other = [];
        fallback.other.push(window);
      }
    }
    models.Codex = fallback;
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

export function applyCodexModelFilter(models: CodexModelEntry[], allowed?: string[]): CodexModelEntry[] {
  if (!allowed || allowed.length === 0) return models;
  return models.filter((m) => allowed.includes(m.name));
}

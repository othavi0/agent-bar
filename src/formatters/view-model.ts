import type { ProviderQuota } from '../providers/types';
import { loadSettingsSync, type Settings, type WindowPolicy } from '../settings';
import { applyCodexModelFilter, type CodexModelEntry, codexModelsFromQuota } from './codex-helpers';

/** Codex view data resolved from settings — what the Codex builder needs. */
export interface CodexViewModel {
  models: CodexModelEntry[];
  policy: WindowPolicy;
}

/**
 * Derive the Codex view model from already-loaded settings.
 *
 * The settings source is the caller's choice — `resolveCodexViewModel` loads
 * fresh, while the Waybar hot path passes settings from its 5s cache. The
 * derivation (window policy + model filter) lives here so both share it.
 */
export function resolveCodexViewModelFrom(settings: Settings, p: ProviderQuota): CodexViewModel {
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? 'both';
  const models = applyCodexModelFilter(codexModelsFromQuota(p), settings.models?.[p.provider]);
  return { models, policy };
}

/** Resolve the Codex view model: filtered models + window policy, from fresh settings. */
export function resolveCodexViewModel(p: ProviderQuota): CodexViewModel {
  return resolveCodexViewModelFrom(loadSettingsSync(), p);
}

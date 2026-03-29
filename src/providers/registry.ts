import type { Provider } from './types';

const registry: Provider[] = [];

/**
 * Register a provider. Called at module scope by each provider file.
 */
export function registerProvider(provider: Provider): void {
  if (registry.some((p) => p.id === provider.id)) return;
  registry.push(provider);
}

/**
 * Get all registered providers (in registration order).
 */
export function getRegisteredProviders(): readonly Provider[] {
  return registry;
}

/**
 * Get registered provider IDs.
 */
export function getRegisteredProviderIds(): string[] {
  return registry.map((p) => p.id);
}

/**
 * Get provider by ID from registry.
 */
export function getRegisteredProvider(id: string): Provider | undefined {
  return registry.find((p) => p.id === id);
}

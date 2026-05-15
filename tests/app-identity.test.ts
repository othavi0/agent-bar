import { describe, expect, it } from 'bun:test';
import {
  APP_HIDDEN_CLASS,
  APP_NAME,
  TERMINAL_HELPER_NAME,
  WAYBAR_MODULE_PREFIX,
  WAYBAR_NAMESPACE,
  WAYBAR_SELECTOR_PREFIX,
} from '../src/app-identity';

describe('app identity', () => {
  it('uses agent-bar as the canonical public namespace', () => {
    expect(APP_NAME).toBe('agent-bar');
    expect(WAYBAR_NAMESPACE).toBe('agent-bar');
    expect(WAYBAR_MODULE_PREFIX).toBe('custom/agent-bar-');
    expect(WAYBAR_SELECTOR_PREFIX).toBe('#custom-agent-bar-');
    expect(APP_HIDDEN_CLASS).toBe('agent-bar-hidden');
    expect(TERMINAL_HELPER_NAME).toBe('agent-bar-open-terminal');
  });
});

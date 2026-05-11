import { describe, expect, it } from 'bun:test';
import { COPILOT_MISSING_ERROR, findCopilotBin, getCopilotCandidatePaths } from '../src/copilot-cli';

describe('copilot-cli helpers', () => {
  it('returns the current missing-cli hint', () => {
    expect(COPILOT_MISSING_ERROR).toContain('GitHub Copilot CLI');
  });

  it('checks common install paths under HOME', () => {
    expect(getCopilotCandidatePaths('/tmp/agent-bar-home')).toEqual([
      '/tmp/agent-bar-home/.local/bin/copilot',
      '/tmp/agent-bar-home/.cache/.bun/bin/copilot',
      '/tmp/agent-bar-home/.bun/bin/copilot',
    ]);
  });

  it('prefers copilot from PATH when available', () => {
    const found = findCopilotBin({
      home: '/tmp/agent-bar-home',
      which: () => '/usr/local/bin/copilot',
      exists: () => false,
    });

    expect(found).toBe('/usr/local/bin/copilot');
  });

  it('falls back to known install locations', () => {
    const found = findCopilotBin({
      home: '/tmp/agent-bar-home',
      which: () => null,
      exists: (path) => path === '/tmp/agent-bar-home/.local/bin/copilot',
    });

    expect(found).toBe('/tmp/agent-bar-home/.local/bin/copilot');
  });

  it('returns null when copilot is unavailable', () => {
    const found = findCopilotBin({
      home: '/tmp/agent-bar-home',
      which: () => null,
      exists: () => false,
    });

    expect(found).toBeNull();
  });
});

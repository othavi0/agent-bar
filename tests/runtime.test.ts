import { afterEach, describe, expect, it } from 'bun:test';
import { isCompiledBinary } from '../src/runtime';

describe('isCompiledBinary', () => {
  afterEach(() => {
    delete process.env.AGENT_BAR_FORCE_COMPILED;
  });

  it('is false when running via bun (not a compiled binary)', () => {
    expect(isCompiledBinary()).toBe(false);
  });

  it('is true when the compiled-binary override is set', () => {
    process.env.AGENT_BAR_FORCE_COMPILED = '1';
    expect(isCompiledBinary()).toBe(true);
  });
});

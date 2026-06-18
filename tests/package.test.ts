import { describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import pkg from '../package.json';

const repoRoot = join(import.meta.dir, '..');

describe('npm package contract', () => {
  it('publishes the built CLI as the package entrypoint', () => {
    expect(pkg.name).toBe('@noctuacore/agent-bar');
    expect(pkg.main).toBe('dist/index.js');
    expect(pkg.bin).toEqual({ 'agent-bar': 'scripts/agent-bar' });
  });

  it('declares the Bun runtime requirement', () => {
    expect(pkg.engines).toEqual({ bun: '>=1.1.0' });
  });

  it('does not ship the CI-only publish helper to consumers', () => {
    expect(pkg.files).not.toContain('scripts/bun-publish-with-npm-token');
  });

  it('publishes only runtime assets and public documentation', () => {
    expect(pkg.files).toEqual([
      'dist/',
      'scripts/agent-bar',
      'scripts/agent-bar-open-terminal',
      'icons/',
      'README.md',
      'LICENSE',
      'CHANGELOG.md',
      'docs/README.md',
      'docs/architecture.md',
      'docs/commands.md',
      'docs/runtime.md',
      'docs/integration.md',
      'docs/waybar-contract.md',
      'docs/troubleshooting.md',
      'docs/new-provider.md',
      'docs/json-output.md',
      'docs/assets/agent-bar-banner.png',
    ]);
  });

  it('exposes release metadata and dry-run checks', () => {
    expect(pkg.repository).toEqual({
      type: 'git',
      url: 'git+https://github.com/othavioquiliao/agent-bar.git',
    });
    expect(pkg.bugs).toEqual({ url: 'https://github.com/othavioquiliao/agent-bar/issues' });
    expect(pkg.homepage).toBe('https://github.com/othavioquiliao/agent-bar#readme');
    expect(pkg.publishConfig).toEqual({ registry: 'https://registry.npmjs.org/', access: 'public' });
    expect(pkg.scripts.prepack).toBe('bun run build');
    expect(pkg.scripts['release:check']).toContain('bun pm pack --dry-run --ignore-scripts');
    expect(pkg.scripts['publish:dry-run']).toBe('bash ./scripts/bun-publish-with-npm-token --dry-run --access public');
    expect(pkg.scripts['publish:npm']).toBe('bash ./scripts/bun-publish-with-npm-token --access public');
  });

  it('keeps the bin shim usable in source checkouts and published packages', () => {
    const shim = readFileSync(join(repoRoot, 'scripts', 'agent-bar'), 'utf8');

    expect(shim).toContain('if [ -f "$APP_DIR/src/index.ts" ]; then');
    expect(shim).toContain('exec bun "$APP_DIR/src/index.ts" "$@"');
    expect(shim).toContain('exec bun "$APP_DIR/dist/index.js" "$@"');
  });

  it('bridges npm login tokens into Bun publish', () => {
    const helper = readFileSync(join(repoRoot, 'scripts', 'bun-publish-with-npm-token'), 'utf8');

    expect(helper).toContain('NPM_CONFIG_TOKEN');
    expect(helper).toContain('^//registry.npmjs.org/:_authToken=');
    expect(helper).toContain('exec bun publish "$@"');
  });
});

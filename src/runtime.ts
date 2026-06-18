/**
 * True when running inside a `bun build --compile` standalone binary.
 * Detected via the `$bunfs` virtual-filesystem prefix that `import.meta.dir`
 * carries in a compiled binary (empirically `/$bunfs/root`) — immune to the
 * binary being renamed, symlinked, or found via PATH. `AGENT_BAR_FORCE_COMPILED=1`
 * is a test seam.
 */
export function isCompiledBinary(): boolean {
  if (process.env.AGENT_BAR_FORCE_COMPILED === '1') return true;
  return import.meta.dir.startsWith('/$bunfs');
}

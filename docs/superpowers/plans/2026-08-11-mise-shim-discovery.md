# Mise Shim Discovery Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provider executable discovery returns the PATH-resolved path as-is (no symlink canonicalization), so mise shims run in shim mode and `amp usage` reaches the real Amp CLI.

**Architecture:** One function changes: `resolve_executable` in `src/providers/catalog.rs` stops calling `canonicalize_best_effort` in both branches (PATH scan and fallback templates). `canonicalize_best_effort` becomes unused and is deleted. A regression test proves a symlink whose target has a different basename (the mise shim shape) is returned as the symlink path, not the target.

**Tech Stack:** Rust (cargo test, tempfile), no new dependencies.

## Global Constraints

- Rust/Cargo and QML only; no Node runtime or test tooling.
- Use temporary plugin roots and isolated XDG directories for tests (tempfile already used in `catalog.rs` tests).
- Do not run live setup, update, uninstall, rescan, shell restart, or config mutation.
- Do not edit `/usr/share/omarchy`.
- Do not touch the installed bundle in `~/.config/omarchy/plugins` (final QA gate only).
- Gates for shared contract changes: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- Commits authorized by the user on 2026-08-11 ("Pode continuar e pode comitar sim!"). No push.

---

### Task 1: Discovery preserves symlink paths (mise shim regression)

**Files:**
- Modify: `src/providers/catalog.rs:314-331` (`resolve_executable`)
- Delete: `src/providers/catalog.rs:365-367` (`canonicalize_best_effort`)
- Test: `src/providers/catalog.rs` (tests module, after `fallback_used_when_path_empty`)

**Interfaces:**
- Consumes: existing test helper `write_exec(path: &Path, executable: bool)` in the same tests module; `discover(&AMP, &env)`.
- Produces: `resolve_executable` keeps its signature `fn resolve_executable(&ProviderDescriptor, &ExecutionEnvironment) -> Result<Option<PathBuf>, CatalogError>`; only the returned path value changes (no callers need edits).

- [ ] **Step 1: Write the failing regression test**

Add to the `tests` module in `src/providers/catalog.rs` (after `fallback_used_when_path_empty`). It reproduces the mise shim shape: PATH contains `amp` as a symlink to a binary named `mise`.

```rust
    #[test]
    fn discovery_returns_symlink_path_not_canonical_target() {
        // Mise shims are symlinks named after the tool pointing at the mise
        // binary. Executing the canonical target changes argv[0] to "mise",
        // which disables shim dispatch, so discovery must preserve the
        // symlink path (design 2026-08-11).
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let path_dir = dir.path().join("shims");
        let target = dir.path().join("tools").join("mise");
        write_exec(&target, true);
        fs::create_dir_all(&path_dir).unwrap();
        let shim = path_dir.join("amp");
        std::os::unix::fs::symlink(&target, &shim).unwrap();
        let env = ExecutionEnvironment {
            home,
            path_dirs: vec![path_dir],
            grok_home: None,
        };
        let discovery = discover(&AMP, &env).unwrap();
        assert_eq!(discovery.collection_executable().unwrap(), shim.as_path());
        assert_eq!(discovery.login_executable().unwrap(), shim.as_path());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test discovery_returns_symlink_path_not_canonical_target`
Expected: FAIL — the assertion reports the canonical `…/tools/mise` path instead of `…/shims/amp`.

- [ ] **Step 3: Implement the fix**

In `resolve_executable`, return the candidate unchanged in both branches, and delete the now-unused `canonicalize_best_effort` function:

```rust
fn resolve_executable(
    descriptor: &ProviderDescriptor,
    env: &ExecutionEnvironment,
) -> Result<Option<PathBuf>, CatalogError> {
    // Return the candidate as found. Following symlinks here breaks version
    // managers such as mise, whose shims are symlinks to the manager binary
    // and rely on argv[0] to dispatch to the real tool (design 2026-08-11).
    for dir in &env.path_dirs {
        let candidate = dir.join(descriptor.executable_name);
        if is_executable_file(&candidate) {
            return Ok(Some(candidate));
        }
    }
    for template in descriptor.fallback_executable_paths {
        let candidate = expand_template(template, env)?;
        if is_executable_file(&candidate) {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}
```

Delete:

```rust
fn canonicalize_best_effort(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
```

Note: `is_executable_file` uses `fs::metadata`, which follows symlinks, so a symlink to an executable still qualifies. The existing test `fallback_used_when_path_empty` compares both sides through `fs::canonicalize` and keeps passing unchanged.

- [ ] **Step 4: Run the full gates**

Run: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all pass. Clippy is the guard that `canonicalize_best_effort` was actually removed (dead code fails `-D warnings`).

- [ ] **Step 5: Commit**

```bash
git add src/providers/catalog.rs
git commit -m "fix: discovery preserves symlink paths so mise shims dispatch"
```

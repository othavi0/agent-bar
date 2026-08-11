# Monorepo Migration (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure `othavi0/agent-bar` on a local branch so the repository
root IS the installable Omarchy plugin tree, with the dist repository's
history grafted in so existing installs fast-forward — everything short of
the GitHub renames (Phase 2, separately authorized).

**Architecture:** Move `assets/omarchy/*` to the repo root (QML source
becomes the shipped QML). Repurpose `agent-bar-bundle` from "assemble a
separate tree" to "stamp release artifacts into the repo root" with an
explicit shipped-file scope. Rework `auto-release.yml` to a single-repo
pipeline. Graft `othavi0/omarchy-agent-bar` history via
`--allow-unrelated-histories` merge so the dist tip is an ancestor of the
new master.

**Tech Stack:** Rust/Cargo, QML (Qt6/Quickshell), Bash, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-11-monorepo-migration-design.md`

## Global Constraints

- All work on branch `feat/monorepo-migration`; commits local only, **never push**.
- Commit subjects: English Conventional Commits, ≤50 chars (project CLAUDE.md).
- No production `unwrap()`/`expect()` (`#![cfg_attr(not(test), deny(...))]` stays).
- `scripts/agent-bar-open-terminal` stays Bash and argv-safe; no `sh -c`/`eval`.
- Checkpoint gates after every task: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`.
- QML gates (Tasks 1, 5, 9): Qt6 binary paths are mandatory —
  `/usr/lib/qt6/bin/qmllint` and `/usr/lib/qt6/bin/qmltestrunner` with
  `QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1`.
- Shipped-file scope (the ONLY files bundle.json may inventory):
  `BarWidget.qml CoreMaintenance.js CoreScroll.js CoreService.js
  CoreSettings.js CoreView.js LICENSE MaintenanceView.qml Popup.qml
  ProviderRail.qml ProviderView.qml README.md Service.qml SettingsView.qml
  manifest.json preview.png bin/agent-bar scripts/agent-bar-open-terminal`
  plus every file under `components/` and `icons/`.
- The dist remote is read-only in this plan: fetch only, never push.

---

### Task 1: Branch + move plugin tree to repo root

**Files:**
- Move: `assets/omarchy/*` → repo root (QML/JS files, `components/`, `icons/`, `manifest.json`)
- Modify: `tests/qml/tst_*.qml` (import paths), `src/notifications/state.rs`,
  `tests/countdown_parity.rs`, `tests/gui_vocabulary.rs`, `tests/icon_assets.rs`,
  `tests/servicecore_contract.rs`, `tests/severity_parity.rs`,
  `tests/update_check_parity.rs`, `tests/active_legacy_scan.rs`,
  `scripts/verify-v10-ui`, `CONTRIBUTING.md`, `CLAUDE.md`, `AGENTS.md`
- Modify: `manifest.json` (root, after move): placeholder → current Cargo version

**Interfaces:**
- Produces: plugin QML at repo root; `manifest.json` at root with a real
  semver version (no `__AGENT_BAR_VERSION__` in the committed tree). Every
  later task assumes this layout.

- [ ] **Step 1: Create the branch**

```bash
git checkout master && git pull --ff-only
git checkout -b feat/monorepo-migration
```

- [ ] **Step 2: Move the tree with git mv (history-preserving)**

```bash
git mv assets/omarchy/BarWidget.qml assets/omarchy/Service.qml \
  assets/omarchy/Popup.qml assets/omarchy/ProviderRail.qml \
  assets/omarchy/ProviderView.qml assets/omarchy/MaintenanceView.qml \
  assets/omarchy/SettingsView.qml assets/omarchy/CoreMaintenance.js \
  assets/omarchy/CoreScroll.js assets/omarchy/CoreService.js \
  assets/omarchy/CoreSettings.js assets/omarchy/CoreView.js \
  assets/omarchy/manifest.json assets/omarchy/components \
  assets/omarchy/icons .
rmdir assets/omarchy
```

- [ ] **Step 3: Stamp the root manifest with the current version**

```bash
VER="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
sed -i "s/__AGENT_BAR_VERSION__/${VER}/" manifest.json
grep '"version"' manifest.json   # must show the real version
```

- [ ] **Step 4: Rewrite every `assets/omarchy` reference**

Mechanical rule: a path `assets/omarchy/X` becomes `X`; a QML test import
`"../../assets/omarchy/CoreView.js"` becomes `"../../CoreView.js"`.
Apply and then prove exhaustion:

```bash
grep -rl 'assets/omarchy' tests src scripts CONTRIBUTING.md CLAUDE.md AGENTS.md \
  | xargs sed -i 's#assets/omarchy/#/#g; s#assets/omarchy#.#g'
# The blunt sed above produces absolute-looking "/X" in some spots; review
# every hunk with `git diff` and fix by hand — the intent is exactly:
#   import "../../assets/omarchy/CoreView.js"  ->  import "../../CoreView.js"
#   "assets/omarchy/BarWidget.qml"             ->  "BarWidget.qml"
#   -import assets/omarchy                     ->  -import .
#   omarchy plugin validate assets/omarchy     ->  omarchy plugin validate .
grep -rn 'assets/omarchy' --exclude-dir=target --exclude-dir=.git \
  --exclude-dir=docs . && echo "LEFTOVERS — fix them" || echo "clean"
```

`docs/specs/v10/**` and `docs/history/**` keep their references (they are
the frozen v10 spec/build record; Task 8 amends only living docs). If
`tests/active_legacy_scan.rs` or `tests/active_docs.rs` enforce doc
wording that now mismatches, update those expectations in the same commit.

- [ ] **Step 5: Run the full Rust + QML gates**

```bash
cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings
find . -maxdepth 1 -name '*.qml' -print0 | xargs -0 ls components/*.qml \
  | true  # sanity: root QMLs present
/usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell \
  ./*.qml components/*.qml
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import . -o -,txt
```

Expected: all green. `cargo test` will FAIL in `dist_tree_validate.rs`
(its fake repo builds `assets/omarchy`) — that failure is expected here
and is fixed by Task 2; do not "fix" it by restoring the directory.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move plugin tree to repo root"
```

---

### Task 2: Repurpose bundle assemble into root stamping

**Files:**
- Modify: `src/plugin/bundle.rs` (BundleBuilder::assemble → stamp; shipped scope)
- Modify: `src/bin/agent-bar-bundle.rs` (CLI verb `stamp`)
- Rename+rewrite: `tests/dist_tree_validate.rs` → `tests/root_tree_validate.rs`
- Modify: `tests/agent_bar_bundle_cli.rs`

**Interfaces:**
- Consumes: root layout from Task 1.
- Produces: `BundleBuilder::stamp(&self, repo_root: &Path, helper_bin: &Path)
  -> Result<BundleReceipt, BundleError>` which (a) copies `helper_bin` to
  `<root>/bin/agent-bar` mode 0755, (b) copies `docs/media/demo.png` to
  `<root>/preview.png` mode 0644, (c) builds the receipt from the shipped
  scope only, (d) writes `<root>/bundle.json`, (e) validates. CLI:
  `agent-bar-bundle stamp source-commit <40-hex>`.
  `pub const SHIPPED_ROOT_FILES: &[&str]` and
  `pub const SHIPPED_DIRS: &[&str] = &["components", "icons"]` exported
  from `bundle.rs` (values = Global Constraints scope).

- [ ] **Step 1: Rewrite the tree test as the failing spec of the new contract**

`git mv tests/dist_tree_validate.rs tests/root_tree_validate.rs`, then
adapt it (same omarchy-validate mirror assertions, new fixture):

- `fake_repo` now materializes the shipped files at the fake repo ROOT
  (copy the real root: `*.qml`, `Core*.js`, `components/`, `icons/`,
  `manifest.json` — plus fakes for `LICENSE`, `README.md`,
  `docs/media/demo.png`, `scripts/agent-bar-open-terminal`) and also
  creates non-shipped noise that must be tolerated and excluded:
  `src/lib.rs`, `docs/dev/notes.md`, `Cargo.toml`.
- Replace `assemble_fake_tree` with:

```rust
fn stamp_fake_root(dir: &Path) -> (PathBuf, agent_bar::plugin::bundle::BundleReceipt) {
    let repo_root = dir.join("repo");
    fake_repo(&repo_root);
    let version = manifest_version(&repo_root); // read from fixture manifest.json
    let helper = dir.join("agent-bar");
    fake_helper(&helper, &version);
    let builder = BundleBuilder::new(version, "0".repeat(40)).unwrap();
    let receipt = builder.stamp(&repo_root, &helper).unwrap();
    (repo_root, receipt)
}
```

- Keep every manifest-grammar assertion from the old test verbatim
  (id grammar, kinds table, entry points, defaultSection, symlink walk).
- New assertions replacing "distribution-repo completeness":

```rust
// Receipt inventories the shipped scope only — never the source tree.
for f in &receipt.files {
    assert!(
        agent_bar::plugin::bundle::SHIPPED_ROOT_FILES.contains(&f.path.as_str())
            || agent_bar::plugin::bundle::SHIPPED_DIRS
                .iter()
                .any(|d| f.path.starts_with(&format!("{d}/"))),
        "non-shipped file in receipt: {}", f.path
    );
}
assert!(receipt.files.iter().all(|f| f.path != "src/lib.rs"));
assert!(receipt.files.iter().any(|f| f.path == "bin/agent-bar"));
assert!(receipt.files.iter().any(|f| f.path == "preview.png"));
```

- `validate_tree_tolerates_root_git_but_not_symlinks` stays, adjusted to
  the stamped root; the symlink probe goes inside `components/`
  (shipped scope) — a symlink under `src/` is NOT validate_tree's job
  (the shell's own validator covers full-tree at install time).

- [ ] **Step 2: Run to verify the intended failure**

`cargo test --test root_tree_validate` — Expected: compile FAIL
(`stamp` and `SHIPPED_ROOT_FILES` do not exist yet).

- [ ] **Step 3: Implement in `src/plugin/bundle.rs`**

Replace `assemble` with `stamp` (keep `BundleBuilder::new` untouched):

```rust
pub const SHIPPED_ROOT_FILES: &[&str] = &[
    "BarWidget.qml", "CoreMaintenance.js", "CoreScroll.js",
    "CoreService.js", "CoreSettings.js", "CoreView.js", "LICENSE",
    "MaintenanceView.qml", "Popup.qml", "ProviderRail.qml",
    "ProviderView.qml", "README.md", "Service.qml", "SettingsView.qml",
    "bin/agent-bar", "manifest.json", "preview.png",
    "scripts/agent-bar-open-terminal",
];
pub const SHIPPED_DIRS: &[&str] = &["components", "icons"];
```

`stamp` body = today's `assemble` minus output-dir creation, minus
`copy_asset_tree` (QML already lives at root; no version substitution —
delete `MANIFEST_VERSION_PLACEHOLDER` and the substitution branch in
`copy_asset_tree`, then delete `copy_asset_tree` itself), minus
README/LICENSE copies (already at root; keep existence checks), keeping:
helper copy → `bin/agent-bar` 0755, `docs/media/demo.png` →
`preview.png` 0644, `normalize_bundle_modes` **restricted to the shipped
scope**, receipt build, atomic `bundle.json` write, final validation.

`collect_inventory(root)` changes contract: iterate `SHIPPED_ROOT_FILES`
+ walk `SHIPPED_DIRS`, erroring on a missing shipped file, ignoring
everything else. `validate_tree` keeps receipt/inventory equality within
that scope; `reject_special_files` walks only shipped paths plus repo
root's immediate entries excluding `.git` (rationale in Step 1).
`validate_manifest_matches` unchanged. Update the module doc comment: the
receipt is now a release stamp of the repo root, not a separate tree.

- [ ] **Step 4: Update the CLI**

`src/bin/agent-bar-bundle.rs`: verb `assemble` → `stamp`; grammar
`agent-bar-bundle stamp source-commit <40-hex>`; drop the `output` key;
`run_stamp` calls `builder.stamp(&repo_root, &helper)` and prints
`stamped <root>`. `repo_root()` detection: `Cargo.toml` + `manifest.json`
at the same level (replaces the `assets/omarchy` probe). Update
`tests/agent_bar_bundle_cli.rs` expectations to the new grammar/usage
string.

- [ ] **Step 5: Run tests to verify they pass**

`cargo test` — Expected: PASS across the suite (including the reworked
`root_tree_validate` and CLI tests).

- [ ] **Step 6: Gates + commit**

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && git diff --check
git add -A
git commit -m "feat: stamp release artifacts into repo root"
```

---

### Task 3: Teach cut-release to stamp manifest.json

**Files:**
- Modify: `scripts/agent-bar-cut-release` (after the Cargo.toml bump)
- Test: `scripts/check-version` needs no change (already cross-checks
  `manifest.json`/`bundle.json`/helper when given a plugin dir)

**Interfaces:**
- Consumes: root `manifest.json` with a real version (Task 1).
- Produces: cut-release bumps Cargo.toml, Cargo.lock AND root
  `manifest.json` to `$NEXT` in one run; `check-version vX.Y.Z .`
  becomes the release-identity gate.

- [ ] **Step 1: Add the stamp to the script** — insert directly after the
  `cargo update --workspace --offline --quiet` line:

```bash
# Root manifest carries the released version (single-repo layout).
sed -i "s/\"version\": \"${CUR}\"/\"version\": \"${NEXT}\"/" manifest.json
grep -q "\"version\": \"${NEXT}\"" manifest.json || {
  echo "manifest.json version stamp failed" >&2
  exit 1
}
```

- [ ] **Step 2: Verify by dry-run + rehearsal**

```bash
scripts/agent-bar-cut-release --dry-run   # prints next version, touches nothing
git stash --include-untracked --keep-index >/dev/null 2>&1 || true
scripts/agent-bar-cut-release             # real run on the branch
scripts/check-version "v$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')" . \
  || echo "helper mismatch is EXPECTED (bin/agent-bar absent until Task 6 graft)"
git checkout -- Cargo.toml Cargo.lock CHANGELOG.md manifest.json
rm -f docs/releases/"$(ls docs/releases | sort -V | tail -1)"  # remove rehearsal notes ONLY if just created
git status --porcelain                    # must be clean again
```

- [ ] **Step 3: ShellCheck + commit**

```bash
shellcheck scripts/agent-bar-cut-release scripts/check-version
git add scripts/agent-bar-cut-release
git commit -m "feat: stamp manifest version at release cut"
```

---

### Task 4: Single-repo auto-release workflow

**Files:**
- Modify: `.github/workflows/auto-release.yml` (full rewrite below)

**Interfaces:**
- Consumes: `agent-bar-bundle stamp` (Task 2), cut-release stamping (Task 3).
- Produces: one `release: vX.Y.Z` commit per release on `master`
  containing bumps + `bin/agent-bar` + `bundle.json` + `preview.png`;
  no dist push, no deploy key.

- [ ] **Step 1: Replace the workflow with:**

```yaml
name: Auto release

# Every product merge to master cuts an official release. The release
# commit itself carries the stamped plugin artifacts (bin/agent-bar,
# bundle.json, manifest version), so the repository root is always the
# complete installable plugin tree and `omarchy plugin update` receives
# exactly one fast-forward commit per release.

on:
  push:
    branches: [master]
    paths:
      - 'src/**'
      - 'scripts/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - '*.qml'
      - 'Core*.js'
      - 'components/**'
      - 'icons/**'
      - 'manifest.json'
  workflow_dispatch:

permissions:
  contents: write

concurrency:
  group: auto-release
  cancel-in-progress: false

jobs:
  release:
    runs-on: ubuntu-latest
    if: >-
      github.event_name == 'workflow_dispatch' ||
      !startsWith(github.event.head_commit.message, 'release:')
    steps:
      - name: Checkout
        uses: actions/checkout@v7
        with:
          fetch-depth: 0

      - name: Install Rust (gnu host target)
        run: |
          rustup toolchain install stable --profile minimal
          rustup default stable
          rustup target list --installed

      - name: Fetch dependencies
        run: cargo fetch

      - name: Cut release (bump + notes + manifest stamp)
        id: cut
        run: |
          set -euo pipefail
          scripts/agent-bar-cut-release
          VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
          echo "version=${VERSION}" >> "$GITHUB_OUTPUT"

      - name: Rust gates
        run: |
          cargo fmt --check
          cargo test
          cargo clippy --all-targets -- -D warnings

      - name: Build release helper (x86_64-unknown-linux-gnu)
        run: cargo build --release --target x86_64-unknown-linux-gnu

      - name: Stamp plugin artifacts into the repo root
        run: |
          set -euo pipefail
          SOURCE_COMMIT="$(git rev-parse HEAD)"
          mkdir -p target/release
          cp -f target/x86_64-unknown-linux-gnu/release/agent-bar target/release/agent-bar
          cargo run --bin agent-bar-bundle -- stamp source-commit "$SOURCE_COMMIT"
          scripts/check-version "v${{ steps.cut.outputs.version }}" .

      - name: Root inventory / modes / architecture
        run: |
          set -euo pipefail
          test -f bundle.json
          test -f manifest.json
          test -x bin/agent-bar
          test -x scripts/agent-bar-open-terminal
          file bin/agent-bar | grep -E 'ELF 64-bit|x86-64|x86_64'
          bin/agent-bar version
          # QML/Quattro gates consume accepted checkpoint evidence on
          # Omarchy hosts; Ubuntu runners have no Omarchy runtime.

      - name: Commit, tag, push, publish
        env:
          GH_TOKEN: ${{ github.token }}
          VERSION: ${{ steps.cut.outputs.version }}
        run: |
          set -euo pipefail
          git config user.name "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add Cargo.toml Cargo.lock CHANGELOG.md "docs/releases/${VERSION}.md" \
            manifest.json bundle.json preview.png bin/agent-bar
          git commit -m "release: v${VERSION}"
          git tag "v${VERSION}"
          git push origin HEAD:master "refs/tags/v${VERSION}"
          gh release create "v${VERSION}" \
            --title "Agent Bar ${VERSION}" \
            --notes-file "docs/releases/${VERSION}.md"
```

- [ ] **Step 2: Static checks**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/auto-release.yml'))" \
  || ruby -ryaml -e "YAML.load_file('.github/workflows/auto-release.yml')"
git diff --check
```

Also re-read the diff for the two intentional contract changes: the skip
guard now matches `release:` (the new single commit) and the trigger
paths cover root QML/JS instead of `assets/**`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/auto-release.yml
git commit -m "ci: release from single repository"
```

---

### Task 5: Root README, preview, and assets/dist removal

**Files:**
- Rewrite: `README.md` (root)
- Create: `preview.png` (root, copied from `docs/media/demo.png`)
- Delete: `assets/dist/` (directory becomes empty of purpose; `assets/` disappears)

**Interfaces:**
- Consumes: none. Produces: marketplace-complete root (README + LICENSE +
  preview + real-version manifest) that `omarchy plugin validate .` accepts.

- [ ] **Step 1: Rewrite README.md** — structure (English, user-first;
  merge today's `assets/dist/README.md` install copy with the product
  overview from the current root README):

```markdown
# Agent Bar

<what it is — current dist README intro paragraph>

![Agent Bar preview](preview.png)

## Install / ## Update / ## Remove
<the three omarchy plugin commands, verbatim from assets/dist/README.md,
 with the install URL https://github.com/othavi0/omarchy-agent-bar.git>

## How it works / ## Providers
<condensed from the current root README>

## Development
This repository is both the plugin (root tree) and its source. See
docs/dev/architecture.md, docs/dev/releasing.md, CONTRIBUTING.md.
`bin/agent-bar` and `bundle.json` are release artifacts committed by CI —
never edit them by hand.

## License
MIT. See LICENSE.
```

- [ ] **Step 2: Preview + deletion**

```bash
cp docs/media/demo.png preview.png
git rm -r assets/dist
rmdir assets 2>/dev/null || true
git add README.md preview.png
```

- [ ] **Step 3: Validate + gates**

```bash
omarchy plugin validate .   # root must now pass the shell's validator
cargo test                  # active_docs / language gates over the new README
git diff --check
```

Note: if `omarchy plugin validate .` trips on a symlink under `target/`
(the validator walks the whole tree; `target/` is local-only noise), run
it against a clean copy of the committed tree instead:

```bash
TMP="$(mktemp -d)"; git archive HEAD | tar -x -C "$TMP"
omarchy plugin validate "$TMP"; rm -rf "$TMP"
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "docs: single user-facing root README"
```

---

### Task 6: Graft the distribution history

**Files:**
- Merge: remote `dist` (`https://github.com/othavi0/omarchy-agent-bar.git`) `master`
- Arrive from theirs: `bin/agent-bar`, `bundle.json`

**Interfaces:**
- Consumes: root layout matching the dist layout (Tasks 1–5).
- Produces: dist tip is an ancestor of the branch → existing installs
  fast-forward. `bin/agent-bar` (v10.3.8 binary) and `bundle.json`
  (v10.3.8 receipt) committed at root until the first monorepo release
  replaces them.

- [ ] **Step 1: Fetch and merge**

```bash
git remote add dist https://github.com/othavi0/omarchy-agent-bar.git 2>/dev/null || true
git fetch dist master
git merge --allow-unrelated-histories --no-commit dist/master || true
git status --short   # inspect: expect both-added conflicts only
```

Resolution rule — ours for everything except the two release artifacts:

```bash
git checkout --theirs -- bin/agent-bar bundle.json
git ls-files -u | awk '{print $4}' | sort -u | grep -v -e '^bin/agent-bar$' -e '^bundle.json$' \
  | xargs -r git checkout --ours --
git add -A
git merge --continue   # or: git commit -m "chore: graft distribution history"
```

Subject if prompted: `chore: graft distribution history`

- [ ] **Step 2: Prove the fast-forward property (the point of this task)**

```bash
git merge-base --is-ancestor dist/master HEAD && echo "FF OK" || echo "FF BROKEN — stop"
test -x bin/agent-bar && bin/agent-bar version   # prints 10.3.8
python3 -c "import json;print(json.load(open('bundle.json'))['version'])"  # 10.3.8
```

Expected: `FF OK`, helper runs, receipt v10.3.8. The version mismatch
between `manifest.json` (current Cargo version) and `bundle.json`
(10.3.8) is expected until the first monorepo release; `check-version`
is a release-time gate, not a standing test, precisely for this reason.

- [ ] **Step 3: Full gates**

```bash
cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check
```

---

### Task 7: Move the release-notes URL to the final repo name

**Files:**
- Modify: `src/plugin/maintenance.rs:35` (`RELEASE_NOTES_URL_PREFIX`)
- Test: `tests/update_check_parity.rs`, `src/plugin/maintenance.rs` unit tests

**Interfaces:**
- Consumes: nothing new. Produces:
  `pub const RELEASE_NOTES_URL_PREFIX: &str = "https://github.com/othavi0/omarchy-agent-bar/releases/tag/v";`
  `DIST_RECEIPT_URL` stays byte-identical (that URL survives the rename).

- [ ] **Step 1: Update the failing expectation first** — change every
  literal `https://github.com/othavi0/agent-bar/releases/tag/v` in tests
  (maintenance.rs `#[cfg(test)]` block lines ~625, ~754, and
  `tests/update_check_parity.rs` if present) to
  `https://github.com/othavi0/omarchy-agent-bar/releases/tag/v`.

- [ ] **Step 2: Run to verify failure** — `cargo test maintenance` —
  Expected: FAIL on the prefix assertions.

- [ ] **Step 3: Flip the constant** at `src/plugin/maintenance.rs:35`.

- [ ] **Step 4: `cargo test`** — Expected: PASS. GitHub's rename redirect
  keeps old binaries' notes links working; new binaries use the final name.

- [ ] **Step 5: Commit**

```bash
git add src/plugin/maintenance.rs tests/
git commit -m "feat: release notes URL on final repo name"
```

---

### Task 8: Documentation and contract amendments

**Files:**
- Create: `docs/adr/0006-single-repository-distribution.md`
- Rewrite: `docs/dev/releasing.md`
- Modify: `CLAUDE.md`, `AGENTS.md`, `docs/dev/architecture.md`,
  `docs/guide/runtime.md`, `CONTRIBUTING.md` (whatever Task 1's grep
  left pointing at the two-repo model)

**Interfaces:** none; prose only, English.

- [ ] **Step 1: Write ADR 0006** — content skeleton:

```markdown
# 0006 — Single-repository distribution

Status: accepted. Supersedes the two-repository mechanics of ADR 0005
(the release-on-every-product-merge policy of 0005 stands).

Context: `omarchy plugin add/update` consumes a git repo whose root is
the plugin tree, fast-forward only. v10 shipped that tree from a
separate artifact repo; the community-facing source repo was invisible
to installs, and the split cost a deploy key, an append-only mirror,
and a README pointing users elsewhere.

Decision: the repository root IS the plugin tree. CI stamps release
artifacts (bin/agent-bar, bundle.json, manifest version) into a single
`release: vX.Y.Z` commit on master. The dist history was grafted as an
ancestor so existing installs fast-forward. master is append-only,
protected against force-push, forever.

Consequences: contributors clone release binaries (~4 MB/release);
master tip between merge and release commit briefly pairs new source
with the previous binary (minutes; the post-merge checklist guards the
red-run case); the interactive update diff includes source changes.
```

- [ ] **Step 2: Rewrite `docs/dev/releasing.md`** keeping the structure
  the file has today but describing the Task 4 pipeline; keep the
  "Update-path verification" checklist (steps unchanged — same commands,
  no dist-repo step 2; replace it with "confirm master gained exactly one
  `release:` commit"), keep the append-only rule section but pointed at
  this repository's master, delete the deploy-key section, keep the
  manual-boundary section (minor/major via workflow_dispatch), and update
  local reproduction to
  `cargo run --bin agent-bar-bundle -- stamp source-commit "$(git rev-parse HEAD)"`.

- [ ] **Step 3: Amend `CLAUDE.md` + `AGENTS.md`** — in CLAUDE.md:
  the Quattro contract bullets (install URL
  `https://github.com/othavi0/omarchy-agent-bar.git`, root layout, no
  dist repo), the Verification section's QML commands
  (`-import .`, `omarchy plugin validate <clean copy>` and the
  `cargo test --test root_tree_validate` name), and the bundle-change
  sentence pointing at `docs/specs/v10/08-plugin-bundle-and-release.md`
  gains "(amended by ADR 0006 / the 2026-08-11 monorepo spec)".

- [ ] **Step 4: Gates + commit** — `cargo test` (active_docs,
  active_language, legacy scan all re-read these files), then:

```bash
git add -A
git commit -m "docs: single-repo distribution contract"
```

---

### Task 9: End-to-end rehearsal (install + update, no live risk)

**Files:** none (temp dirs only). This is the Phase-1 exit gate.

- [ ] **Step 1: Fresh-install rehearsal** — clone the BRANCH as a user would:

```bash
TMP="$(mktemp -d)"
git clone --branch feat/monorepo-migration "$(pwd)" "$TMP/othavi0.agent-bar"
omarchy plugin validate "$TMP/othavi0.agent-bar"
"$TMP/othavi0.agent-bar/bin/agent-bar" version   # 10.3.8 (grafted binary)
```

Expected: validate passes, helper executes.

- [ ] **Step 2: Existing-install update rehearsal** — replay exactly what
  `omarchy-plugin-update` does against a clone that starts at the dist tip:

```bash
git clone https://github.com/othavi0/omarchy-agent-bar.git "$TMP/existing"
git -C "$TMP/existing" remote set-url origin "$(pwd)"
git -C "$TMP/existing" fetch origin feat/monorepo-migration
git -C "$TMP/existing" merge --ff-only FETCH_HEAD && echo "UPDATE FF OK"
omarchy plugin validate "$TMP/existing" && echo "POST-UPDATE VALIDATE OK"
rm -rf "$TMP"
```

Expected: `UPDATE FF OK` then `POST-UPDATE VALIDATE OK`. This is the
proof the spec's graft mechanism works for real installs.

- [ ] **Step 3: Full checkpoint gates one last time**

```bash
cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings
/usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell ./*.qml components/*.qml
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import . -o -,txt
shellcheck scripts/agent-bar-open-terminal scripts/agent-bar-cut-release scripts/check-version
git diff --check
```

- [ ] **Step 4: Report** — summarize branch state, rehearsal evidence, and
  stop. Phase 2 (renames, first v10.4.0 release, marketplace) requires
  fresh explicit authorization and is out of this plan.

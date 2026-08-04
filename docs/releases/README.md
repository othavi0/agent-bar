# Release notes

Author a markdown file per published version:

```text
docs/releases/<semver>.md
```

Tracked notes for the initial Quickshell-only release:
[`10.0.0.md`](10.0.0.md).

The release builder and `.github/workflows/auto-release.yml` consume this
path. The automatic cut writes the file before committing; the workflow
fails if it is missing.

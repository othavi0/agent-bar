# Release notes

Author a markdown file per published version:

```text
docs/releases/<semver>.md
```

Example: `docs/releases/10.0.0.md`.

The release builder and `publish.yml` consume this path. When the file is
absent at tag publish time, CI materializes notes from the GitHub release body
(or a short placeholder) so the builder path remains satisfiable.

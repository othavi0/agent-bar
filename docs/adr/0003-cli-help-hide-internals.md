# Lean public help; internals still parseable

The CLI mixed primary commands with packager/Waybar paths (`assets`,
`export`, `action-right`) and redundant aliases. We decided on **option A**:
reorganize help (menu, status, config, setup, update, uninstall, doctor);
hide internals from help but **keep them parseable** (Waybar modules and
scripts depend on them); `remove` → `uninstall --yes`; `-t` → `status`. No
Omarchy-only hard cut, and `action-right` is not deleted.

**Considered:** (B) unify action-right into `menu --provider`; (C) default
JSON with Waybar explicit. Deferred to avoid breaking the generated contract.

**Status:** accepted (v8.5.0)

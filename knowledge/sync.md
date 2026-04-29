# Sync Contract

Current state:
- `ovmd sync` virtualizes rule content inside target files as a single Overmind managed block.
- Pack target manifests only need `id`, `path`, and `managed`; legacy `template` fields are ignored.
- Missing target files are created with only the managed block, except new `.cursor/rules/AGENTS.mdc` files also get required `alwaysApply: true` frontmatter above the block.
- Existing target files keep local content outside the block. If no block exists, sync inserts one after YAML frontmatter, after the first heading, or at the top.
- Existing files with an Overmind block and no local content outside legacy scaffold are rewritten to managed content only. Cursor `.mdc` targets keep existing frontmatter outside the block.

Desync behavior:
- `ovmd desync` removes Overmind blocks without deleting config.
- Files are deleted only when the remaining content after block removal is empty or recognized legacy Overmind scaffold.
- Cursor frontmatter is preserved when it remains after removing the Overmind block.
- Files with local content outside the block are preserved.
- `ovmd unlink` remains a hidden compatibility alias for `ovmd desync`.

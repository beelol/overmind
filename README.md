# Overmind

`ovmd` syncs modular AI agent rules into projects.

The CLI is separate from rule sources. By default it reads `git@github.com:beelol/rules.git` on `master`, but any project can use a local folder, local file, raw HTTP URL, git URL, or submodule path.

## Install

From source:

```bash
cargo install --git git@github.com:beelol/overmind.git ovmd
```

From release binaries:

```bash
curl -fsSL https://raw.githubusercontent.com/beelol/overmind/master/scripts/install.sh | bash
```

## Basic Usage

```bash
ovmd init
ovmd sync
```

Test local edits from a sibling rules checkout:

```bash
ovmd sync --source ../rules --watch
```

Use a project submodule:

```bash
git submodule add -b master git@github.com:beelol/rules.git .agent-rules/universal
ovmd init --source .agent-rules/universal
ovmd sync --watch
```

## Source Precedence

Overmind resolves source settings in this order:

1. CLI flags
2. project `.overmind.toml`
3. global `~/.config/overmind/config.toml`
4. built-in default `git@github.com:beelol/rules.git`

Example project config:

```toml
[source]
uri = "git@github.com:beelol/rules.git"
ref = "master"
pack = "universal"

[sync]
targets = ["agents", "claude", "gemini", "cursor", "cursor-legacy", "cline", "antigravity"]
```

## Source Cache

Remote git and HTTP sources are cached under:

```text
~/.cache/overmind/sources/<slug>-<hash>/
```

The directory name is derived from normalized `uri + ref + pack`.

Local folders, local files, and project submodules bypass cache and are read directly.

Inspect the active source:

```bash
ovmd doctor
ovmd source path
```

## Editing Rules

Edit the configured source safely:

```bash
ovmd source edit
ovmd pack build
ovmd source publish -m "Update universal agent rules"
```

For local testing before publishing:

```bash
ovmd sync --source ../rules --watch
```

---
name: open-pr
description: Open pull requests in TestBox's preferred style -- concise, technical, no hallucinated why. Use when the user asks to open a PR, create a pull request, write the PR description, draft a PR, or push the current branch up for review.
---

# Open Pull Requests

A PR description's job is to make the diff reviewable, not to restate it. This skill produces concise, technical PR bodies that lead with the *delta in intent* and list the actual tests that were run.

## Core principles

1. **PR = technical audit trail; the ticket carries the functional context.** The Linear ticket explains *why the feature exists*. The PR explains *why the code looks the way it does*. A one-sentence functional opener is fine to orient a reviewer who hasn't clicked through to Linear -- but don't paraphrase the ticket in depth. Link to it for everything else.
2. **Lead with the delta in intent, not the diff.** The reviewer sees the diff already. The body's job is what was true before, what is true after, and why this approach over the alternative.
3. **Match length to change size.** A six-line bug fix gets two sentences. A refactor that moves three modules gets a paragraph. If the description is longer than the diff is interesting, it is wrong.
4. **Never hallucinate the why.** Only the author knows the real motivation. If the why is not inferable from the diff, branch name, commits, or linked ticket, ask the user one targeted question rather than invent one. Leaving the section blank is better than guessing.
5. **No filler openers.** Strip "This PR introduces...", "In this change we...", "The purpose of this PR is...". Start with the verb.
6. **No empty sections.** If a template section does not apply, write a one-line explicit "N/A -- <reason>" or omit the heading entirely. Do not write filler prose under it.
7. **List the tests you actually ran.** Name them -- the test file, the command, what was added, what passed, what was manually reproduced. "Tested locally" with no specifics is worse than nothing. A checkbox alone is not verification. Every PR includes an explicit list of what was run, even if it's one line.
8. **No per-file narration, no walls of bullets.** Group changes by intent, not by file. Three bullets describing *why* beat fifteen bullets describing *what*.
9. **Respect the local template; do not impose a shape.** Read `.github/pull_request_template.md` and fill that. Fall back to a minimal default only when no template exists.
10. **Do not hard-wrap the body.** Write each paragraph as a single line and let GitHub soft-wrap it. Manual breaks at ~72 chars render as ragged half-width lines in the PR UI. (The ~72-char wrap a repo's `AGENTS.md` / `CONTRIBUTING` may specify is a *commit-message* rule; it does not apply to PR bodies.)

## Title rules

Conventional Commits format: `type(scope): subject in imperative mood`.

- **Type** is one of: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `infra`.
- **Scope** is optional and topic-based (`replay`, `executor`, `crawler`, `api`). Pick one when it sharpens the subject; omit when the type alone is clear.
- **Subject** in imperative mood (`add`, not `added` / `adds`).
- **Hard ceiling: 72 characters.** Target ~50 so the title fits narrow panels and `git log --oneline` without wrapping.
- **Never** put Linear IDs in the title or scope. The branch already carries the ticket; the body links it explicitly.
- **Never** prefix with `[TEAM]` or use ALL-CAPS.

Good:

- `fix: handle null lastSeenAt when computing replay window`
- `feat(replay): step through multi-step replays one event at a time`
- `refactor(executor): merge retry helper into the shared module`

Bad:

- `[MON-170] Add stepReplay function` -- Linear ID in subject, generic mechanism phrasing.
- `Updated some files` -- vague, useless in a list view.
- `feat: This change implements a new system for handling multi-step replay events for debugging purposes` -- filler opener, far over 72 chars.

## Template detection

Before drafting, check for a local PR template in this order:

1. `.github/pull_request_template.md`
2. `.github/PULL_REQUEST_TEMPLATE.md`
3. `.github/PULL_REQUEST_TEMPLATE/*.md` -- if multiple files exist in this directory, ask the user once which to use.

If a template is found, use it as the structural skeleton. **Strip the italic placeholder prose** that TestBox templates include under each heading (e.g. `_Think of this more at a technical level..._`). That text is guidance for the human author; it must not appear in the shipped body.

If no template is found, use the default skeleton below.

## Default skeleton (no template)

```markdown
<one-sentence functional opener>

### What changed

<delta in intent>

### Tests

<command, test file, or manual repro>

### Risk / rollout (omit if none)
```

## Section-by-section guidance

### `### What did I change` -- the high-leverage section

Open with **one sentence** that names the user-facing or system-level outcome -- drawn from the linked Linear ticket (see Branch and Linear inference). Then write the delta in intent:

- What was true before.
- What is true after.
- Why this approach over the alternative.

One short paragraph plus at most three bullets for non-obvious decisions (architectural choice, library swap, intentional tradeoff). Cap at one paragraph for focused changes; allow two for refactors that span multiple modules. Steer hard away from per-file narration.

### `### Tests` -- additive section, always include

TestBox's template only has a `Did you...` checkbox for verification, which is too weak on its own. **Insert this section after `### What did I change`** for every PR, even template-driven ones.

List the actual tests run: file paths, commands, what was added, what was reproduced manually. One line is fine for trivial changes. Acceptable shapes:

- `pnpm test src/replay/computeReplayWindow.test.ts` -- added 2 cases for null/undefined `lastSeenAt`, both pass.
- Manual repro on staging: created a fresh session via the SSO script, confirmed the replay job no longer crashes.
- Backed by existing coverage in `replay.test.ts` (no new tests added; behavior unchanged).

If the agent did not run any tests this session, say so explicitly (`No tests run this session -- covered by existing X.test.ts`) rather than fabricating a list.

### `### Demo`

If the diff touches UI (`*.tsx`, `*.vue`, `*.svelte`, `*.css`, frontend template files), prompt the user once for a screenshot or recording and leave a `[screenshot]` placeholder in the body. If no UI change, write exactly: `N/A -- no UI change.` Never fabricate UI descriptions.

### `### QA Notes`

Fill only when the change touches shared code, refactors logic adjacent to other features, or affects migrations / feature flags. Otherwise write exactly: `Scope is self-contained -- test the ticket flow.`

### `### Related Tickets`

The branch name auto-links the main ticket. Use this section only for `Blocked by:`, `Includes:`, or related-but-separate tickets. Omit the section entirely if none apply.

### `### Did you...` checklist

Mark items the agent actually performed this session:

- `test the code locally` -- mark if any tests were run or a manual repro was performed.
- `run unit tests and updated to account for the changes` -- mark if unit tests were run/added.
- `lint the code` -- mark if a linter was run.
- `format the code` -- mark if a formatter was run.

**Never auto-mark**: `test the code on staging`, `add feature flags for new functionality`, `analyze database implications of your changes`. These are user judgments. Leave them unchecked for the user to tick.

## Branch and Linear inference

Extract the ticket ID from the branch with regex `[A-Z]+-\d+` (covers `MON-170`, `multi-step-replay-MON-170`, `MON-170-foo`).

**Always fetch the ticket** via the available Linear MCP connector (`mcp__*__get_issue` or the `linear` connector) -- its title and description shape the one-sentence functional opener and confirm the *why* without asking the user.

If no Linear connector is available, surface that to the user (don't silently skip) and ask once for the ticket context in chat.

Use the ticket only to understand intent and write the opener. **Never paste the ticket body into the PR.**

If the branch has no ticket ID at all, ask the user once whether one applies before drafting.

## Workflow

1. **Detect the template.** Read `.github/pull_request_template.md` (and the `PULL_REQUEST_TEMPLATE.md` / `PULL_REQUEST_TEMPLATE/*.md` variants). If a directory has multiple templates, ask the user once which to use. If none exist, use the default skeleton.
2. **Gather inputs.** Run:
   - `git diff $(git merge-base HEAD origin/main)..HEAD`
   - `git log $(git merge-base HEAD origin/main)..HEAD --format=%s`
   - `git branch --show-current`
   - `gh pr view --json title,body,url` -- if this returns an existing PR, capture its current body.
3. **Infer the ticket ID** from the branch with `[A-Z]+-\d+`. If found, always fetch the ticket via the available Linear MCP connector -- its title/description shapes the one-sentence functional opener in `### What did I change`. If no Linear connector is available, surface this to the user and ask for the ticket context in chat rather than guessing. Never paste the ticket body into the PR.
4. **Size the description to the change.** Skim the diff: one-liner fix, focused feature, or larger refactor. Pick the shortest shape that captures the delta in intent.
5. **Draft in chat first.** Show the user the title and body. Do not run `gh pr create` or `gh pr edit` yet.
6. **Ask one targeted question if -- and only if -- the why is genuinely not inferable.** Never fabricate.
7. **On approval:**
   - If the branch is not pushed to origin, run `git push -u origin HEAD` first.
   - If no PR exists for this branch, run `gh pr create --title "<title>" --body "$(cat <<'EOF'\n<body>\nEOF\n)"`.
   - If a PR already exists, ask once whether to update; on confirmation run `gh pr edit <number> --body "$(cat <<'EOF'\n<body>\nEOF\n)"`.
8. **Return the PR URL** so the user can open it.

## What NOT to include

- AI-attribution footers: no `Co-Authored-By: Claude`, no "Generated with Claude Code" / "Generated with [tool]" lines, no `claude.com` / `anthropic.com` / `cursor.sh` links.
- Emojis. Unicode symbols (✓, ✗, →, ⚠) only if the local repo already uses them.
- Paraphrasing the Linear ticket in depth. A one-sentence functional opener is fine; everything else belongs in Linear (linked).
- Per-file enumeration. The diff already shows which files changed.
- Filler openers ("This PR introduces...", "In this change we...", "The purpose of this PR is...").
- Bare placeholders ("N/A", "None", "--" with no reason). `N/A -- no UI change` is fine; `N/A` alone is not.

## Good examples

### Small backend fix (TestBox template)

**Title:** `fix: handle null lastSeenAt when computing replay window`

```markdown
### What did I change:

Stops the replay job from crashing on freshly-created sessions (MON-170).

`computeReplayWindow` assumed `session.lastSeenAt` was always set, but a session can hit this path before its first heartbeat, throwing on `.getTime()`. Treat a missing `lastSeenAt` as "no prior window" and fall through to the default 24h lookback rather than crashing the job.

### Tests:

- `pnpm test src/replay/computeReplayWindow.test.ts` -- added two cases for `null` and `undefined` `lastSeenAt`, both pass.
- Manual repro: created a fresh session via the SSO script, confirmed the replay job picks it up cleanly on the next cycle.

### QA Notes:

Scope is self-contained -- focus QA on sessions created in the last minute (the only path that produces a null `lastSeenAt`).

### Did you...
- [x] test the code locally?
- [x] run unit tests and updated to account for the changes?
- [x] lint the code?
- [x] format the code?
- [ ] test the code on staging?
```

### Feature with Demo (TestBox template)

**Title:** `feat(replay): step through multi-step replays one event at a time`

```markdown
### What did I change:

Lets engineers debug customer-reported replay issues by inspecting intermediate state (MON-170).

Replay previously emitted all events for a session in a single batch, which made intermediate state invisible. Added a `step` mode that yields one event at a time and exposes the cursor on the existing `ReplaySession` so callers can advance, pause, or jump.

The cursor lives on the session object rather than as a separate handle to avoid threading a new dependency through `useReplay`. The batch mode is preserved as the default so existing callers are unchanged.

### Tests:

- `pnpm test src/replay/` -- added `replay.step.test.ts` (4 cases: advance, pause, reset, end-of-stream). Existing batch tests unchanged.
- `pnpm test src/hooks/useReplay.test.ts` -- added 1 case covering `mode: 'step'`.
- Manual repro: stepped through a 12-event session in the debug UI, verified each event renders before advancing.

### Demo:

[screenshot or recording of the step controls]

### QA Notes:

The cursor uses the same event ordering as batch mode, so existing replay output should be byte-identical when step mode is off. Worth a regression pass on the legacy batch path since the iteration code is now shared.

### Did you...
- [x] test the code locally?
- [x] run unit tests and updated to account for the changes?
- [x] lint the code?
- [x] format the code?
- [ ] test the code on staging?
```

## Bad examples

### Re-describes the diff

```markdown
### What did I change:

I modified `replay.ts` to add a new function called `stepReplay`. I also updated `useReplay.ts` to import this new function and added a `mode` parameter to the `ReplaySession` type in `types.ts`. In `replay.test.ts` I added two new test cases. I also updated `index.ts` to export the new function.
```

Why it fails: narrates the diff file by file. The reviewer can see all of that. Rewrite as the delta in intent: what was true before (single-batch replay), what is true after (cursor-based stepping), and why the cursor lives on the session.

### Filler opener + inflated importance

```markdown
### What did I change:

This PR introduces a critical enhancement to our replay infrastructure that will significantly improve developer experience and debugging capabilities across the platform. The purpose of this change is to enable engineers to more effectively diagnose customer issues by providing fine-grained control over event playback, which has been a long-standing pain point in our tooling...
```

Why it fails: "This PR introduces...", "critical enhancement", "significantly improve" -- pure filler for a single-function addition. Rewrite as: "Added step mode to replay so callers can advance one event at a time. Batch mode is unchanged."

### Wall of bullets / per-file enumeration

```markdown
### What did I change:

- Added `stepReplay` to `replay.ts`
- Added `ReplayMode` enum to `types.ts`
- Added `mode` field to `ReplaySession`
- Added `advance()` method to `ReplaySession`
- Added `pause()` method to `ReplaySession`
- Added `reset()` method to `ReplaySession`
- Updated `useReplay.ts` to accept a `mode` parameter
- Updated `useReplay.ts` to call `advance()` on tick
- Added tests in `replay.test.ts` for step mode
- Added tests in `replay.test.ts` for cursor advance
- Updated `index.ts` to export new symbols
```

Why it fails: twelve bullets, all visible in the diff, zero explanation of why the cursor lives on `ReplaySession` rather than as a separate handle. Rewrite as one paragraph of intent plus at most three bullets for non-obvious decisions.

---
name: address-pr-review
description: Adjust an existing GitHub pull request in response to reviewer feedback with a complete comment ledger, evidence-backed minimal fixes, concise replies, and scope control. Use when asked to address every PR comment, implement requested changes, fix review feedback, respond to or resolve review threads, update a PR after review, or push back on requests that add unrelated complexity or features.
---

# Address PR Review Feedback

Modify an existing PR from review feedback. Account for every comment and change only what supports the feature.

## Establish the feature contract

1. Read the PR title, body, diff, linked requirements, repository instructions, and nearby implementation patterns.
2. State required behavior, preserved interfaces, and explicit non-goals.
3. Treat pre-existing issues and adjacent improvements as out of scope unless the PR makes them unsafe.
4. Ask only when missing intent would materially change scope; otherwise proceed with a conservative stated assumption.

## Build the comment ledger

1. Fetch inline review threads, submitted reviews, and top-level PR conversation comments, including resolved and outdated state.
2. Record every user-authored comment with URL or ID, author, path and line when present, and current state.
3. Act on unresolved, reopened, and newly added feedback. Retain resolved feedback in the ledger so none disappears from accounting. Track bot comments separately.
4. Assign one disposition to every comment:
   - **Fix** — required for correctness, the feature contract, or a demonstrated repository convention.
   - **Explain** — code is correct but intent or evidence is unclear.
   - **Push back** — request expands scope, duplicates existing behavior, adds unused generality, or conflicts with the feature.
   - **Superseded/duplicate** — another change or thread already addresses it.
5. Record the justification, code path, and verification for each disposition before editing. Never silently skip a comment.

Push back with evidence, not preference. Cite the PR contract, existing implementation, dependency or line-count cost, regression risk, or a suitable follow-up boundary.

## Implement narrowly

- Preserve public behavior and interfaces unless an accepted comment requires a change.
- Reuse existing helpers, contracts, and architecture before adding parallel logic.
- Keep fixes traceable to ledger entries; avoid opportunistic refactors.
- Do not add speculative abstractions, configuration, or scaffolding for hypothetical consumers.
- Preserve user changes and commit history. Do not force-push unless explicitly authorized.
- Add or consolidate tests without reducing covered scenarios.

## Validate, publish, and close

1. Run focused checks for each ledger entry, then repository-required build, lint, typecheck, migration, packaging, and UI-parity checks proportionate to risk.
2. Commit and push only validated changes and only when authorized.
3. Reply individually to every active thread with disposition, justification, fixing commit SHA, code path, and exact test reference.
4. Resolve only after the referenced check passes and GitHub writes are authorized.
5. Monitor CI and preview deployment. Fix failures caused by the diff; report unrelated failures with evidence instead of expanding scope.
6. Refetch every comment surface immediately before handoff and address new user-authored feedback.

Report totals for comments, fixes, explanations, pushbacks, superseded items, resolved threads, and remaining work. Never claim completion while a comment lacks a disposition or reference.

## Write safety

If the user requested only a plan or audit, do not edit, commit, push, reply, or resolve. Perform each external write only when the request authorizes it.

## Formatting

If posting to github, ensure the comments have the badge at the start specifying the review was done automatically via an overmind skill with a color, ideally the blue one.

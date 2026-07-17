---
name: review-pr
description: "Deep, concise, read-only review of a GitHub pull request that flags only real feature-relevant issues: correctness bugs, repository convention mismatches, scope creep, dead or duplicated code, and generic inline code that should be shared. Use when asked to review or audit a PR, inspect a PR diff, or draft review findings. Do not use to modify a PR from existing reviewer feedback; use address-pr-review instead."
---

# Review a Pull Request

Review the proposed diff. Do not adjust the branch or address existing review comments.

## Establish intent

1. Read the PR title, body, diff, linked requirements, repository instructions, and nearby implementation patterns.
2. Identify required behavior, preserved interfaces, and explicit non-goals.
3. Ground convention and reuse claims in actual repository evidence.

## Flag only real issues

- Correctness, security, data-loss, concurrency, or compatibility defects.
- Violations of demonstrated repository structure or conventions.
- Duplicated functionality when an existing symbol and path can be cited.
- Dead code, unused scaffolding, or generic abstractions without a current consumer.
- Scope creep, extra knobs, or bells and whistles that do not support the feature.
- Missing verification for behavior changed by the PR.

Ignore pre-existing problems, personal style, speculative cleanup, praise, and unrelated debt. Zero findings is valid.

For every finding, provide path and line, impact, evidence, and the smallest concrete fix. Keep each finding to one or two sentences.

## Pitfalls
Look out for the following pitfalls during your review and request that they are fixed.
- Nested ternaries that should just be if/then/else statements
- Dependencies that don't make sense, like dependency cycles, or infinite re-renders, excessive re-rendering where there should only be one render.
- Switch statements that would be cleaner as a map/dictionary/hashmap/hash (key-value constant time structure)
- Conditionals that would be clear cleaner as a map/dictionary/hashmap/hash (key-value constant time structure)
- Excessive additional features that don't support the primary features. Scan for evidence of what the feature is intended to be.
- Scope creep.
- Nice-to-haves that introduce necessary maintenance and potential regressions that can be done in a follow up or are not necessary to support the feature at all.
- Extra code that was added or duplicated that are a slight deviation from something that already exists and could be used instead from an existing piece of code.
- In-line or in same file utilities that should be generic or are not useful and should be removed.
- Files that are not single-purposed and include lots of changes.
- File structures/code organization that deviates from the repository standard.

## Draft before posting

Present a concise summary and exact inline findings in chat first. Do not post a review or PR comment without explicit authorization.

If authorized, post only the approved findings and return the PR URL. If no findings survive, post one short no-blockers comment rather than an empty review.

## Boundaries

- Do not edit files, commit, push, or resolve review threads under this skill.
- Do not turn review findings into implementation work without a separate user request.
- Use address-pr-review when the task is to modify an existing PR from reviewer feedback.

## Formatting

If posting to github, ensure the comments have the badge at the start specifying the review was done automatically via an overmind skill with a color, ideally the blue one.
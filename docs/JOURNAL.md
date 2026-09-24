# Giacometti Journal Format

Note: this document is originally from https://github.com/lmmx/giacometti/docs/JOURNAL.md

If you are reading this in another repo, it has been copied here to add structure to the
development journalling in this repo. The 'journal' is markdown files under `docs/journal/`.

## Entry Structure

A journal entry is a markdown file in `docs/journal/` named `YYYY-MM-DD-title.md`
with up to four sections (only _Current State_ is required):

1. **Current State** - What is working and implemented
1. **Stubbed** - What exists as placeholder implementations
1. **Missing** - Functionality that has no code yet
1. **Divergence** - README documents behaviour that doesn't exist in code

Notes on sections:

- Distinguish "stubbed" (code exists, returns error) from "missing" (no code)
- "Divergence" compares to documented claims, not aspirations

## Writing Style

Journal entries follow the project communication principles:

### What to state

- State what exists, not what should exist
- Avoid bare constative verbs (exists, is) - describe what the component does or how it's implemented instead
- No recommendations or suggestions
- No evaluation words (better, cleaner, should)
- Use factual present tense only

### Bullet structure

- One statement per bullet
- Each bullet describes a single component's state (a backend, a CLI command, a workflow)
- List related files inline with commas when they implement the same component
- Each bullet is self-contained - no pronouns (this/it), no dependency on other bullets to understand, includes enough context to verify the statement independently

### Provenance and causal chains

- When an entry traces data lineage or a sequence of operations, bullets may reference each other through shared artifacts (filenames, line counts) rather than pronouns.
- Order bullets chronologically within the section to imply sequence.
- To express that one fact explains another, state both facts in a single bullet joined by a dash or "—". Do not split cause and effect across two bullets.

#### Example Do/Don't

- ✅ "`resplit_stratified.py` was run on the checked-in 3119-example splits, not on `convert_to_bio.py` output — the 7-to-3 label coverage change applies to the checked-in data only"
- ❌ "`resplit_stratified.py` reduced labels at zero from 7 to 3" (ambiguous which dataset)
- ❌ "This improvement applies to the checked-in splits" (pronoun, depends on prior bullet)
- ✅ "The checked-in splits contain 808 more examples than `convert_to_bio.py` produces — those 808 examples carry all 8 marker types absent from the `convert_to_bio.py` output"
- ❌ "The 808 additional examples account for the missing types" (requires reader to find the number 808 in a prior bullet)

### File references

- Point to specific files and functions
- File paths in parentheses after statements for verification
- Include line numbers after file paths when referencing specific code (file.rs:45-52)

## Example Do/Don't

- ✅ "Release command not exposed in CLI (main.rs only wraps git commands)"
- ❌ "We should expose the release command in the CLI"
- ❌ "The CLI needs better command structure"

## Example Entry: uv Backends

Filename: `docs/journal/2025-01-25-uv-backends.md`
```markdown
# 2025-01-25: UV Backends

## Current State
- Backend architecture established (git + uv)
- Shell backends functional (git/shell.rs, uv/shell.rs)

## Stubbed
- gitoxide git backend (backends/git/gitoxide.rs)
- rust-crate uv backend (backends/uv/rust_crate.rs)

## Missing
- Release command not exposed in CLI (main.rs only wraps git commands)

## Divergence
- README shows `gcmti release --bump minor` but CLI doesn't accept release subcommand
```

## Example Entry: Release CLI

Filename: `docs/journal/2025-01-28-release-cli.md`
```markdown
# 2025-01-28: Release CLI

## Current State
- main.rs implements release subcommand using clap derive
- CLI accepts --backend flag (shell, gitoxide, github-api) for git operations
- release subcommand calls git::release::run with selected backend (main.rs:45-52)

## Stubbed
- gitoxide git backend selected via CLI but returns "not yet implemented" (backends/git/gitoxide.rs)

## Missing
- CLI backend selection only applies to git operations, uv operations hardcoded to shell backend (main.rs:48)

## Divergence
- README shows GHA usage with `uses: lmmx/giacometti@v1` but no action.yml in repository
```

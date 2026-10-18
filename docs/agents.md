# Agent briefs

Template for delegating a task to an agent, and the rules the orchestrator follows. The rules an agent follows live in `CLAUDE.md`.

## Brief template

Use it for every delegated task and keep the brief itself short.

```
Task: <one roadmap item, one focused piece, about 15 minutes>
Read first: CLAUDE.md, docs/ROADMAP.md, <only the files you need>
Scope: only <crate or folder>; no edits elsewhere except append-only workspace deps.
Deliver: <list>
Tests: each test module starts with `//! Goal:`; table-driven/property tests over single cases.
Docs you write follow the docs rule in CLAUDE.md.
Checks before every non-wip commit: scripts/check.sh fast; before the final report: scripts/check.sh
Commits: commit after every finished module or green run. Prefix unfinished checkpoints with `wip:`. Never add AI attribution.
Report (max 15 lines): files added, public API in one line each, test counts, decisions, uncertainties.
```

## Orchestrator rules

- Isolate each agent in a git worktree; merge only after running `scripts/check.sh` on the merged result.
- Never chain checks through pipes without `set -o pipefail`; a lint failure must stop the commit.
- Validate parsers and importers against real data before calling them done.
- Keep the root Postgres running (`docker compose up -d db`) while agents work: their `check.sh` reuses it instead of starting a compose project of its own, and a file lock lets one test run at a time use it (`sqlx::test` drops other runs' test databases). Remove a stray `agent-*` project with `docker compose -p <name> down -v`.
- Never `cd` into an agent worktree from the orchestrator shell: the harness then makes it the primary directory and forbids returning to the root. Use `git -C <path>` and absolute paths, and merge with `--no-stat` to keep output short.
- After a restart, run `git worktree list` and read each worktree's `git status` before resuming or redoing anything.

### Parallel agents

- Land a foundation task first, with seams so each later agent replaces only its own file (and its own docs bullet, with blank lines around it) and not a shared registry.
- Give each agent a distinct dev-server port.
- Merge the agents one at a time and run the checks after each. Message the agents still running to merge `main` into their branch before they report, so they resolve their own conflicts.

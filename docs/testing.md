# Testing speed

Where the time of a check and a release goes, and the design that tests only what a change can break.

## Measurements

24 cores, `CARGO_BUILD_JOBS=4`, load average 3 to 19 (other agents running).

| Step | Seconds |
| --- | --- |
| Cold test build of the workspace (new worktree) | 328 |
| Warm rebuild after a card file or an engine file changes | 20 to 39 |
| Warm rebuild of the `mtg-testkit` `it` binary after one test file changes | 136 |
| fmt / clippy (warm) / script tests | 3 / 59 / 2 |
| Tests without Postgres (cargo runs the binaries one after another) | 1,211 to 1,242 |
| Tests with Postgres (under the global lock) | 520 to 738 |
| Web (typecheck, lint, format, vitest) | 106 to 121 |
| Release images (server and web, one after the other, after the check) | about 600 |

Test-seconds per binary (nextest, one process per test): `mtg-testkit::it` 18,517 for 17,517 tests, `mtg-protocol::it` 1,650, `mtg-cards::it` 504, all others under 160.

- Each test process loads the built-in library once (`OnceLock`, about 1.1 s), so one process per test (nextest) costs 19,000 test-seconds of loading; libtest in one process pays it once.
- The long tail decides the wall clock: 287 tests over 5 s take 3,478 s, 489 fuzz tests take 2,337 s. Slowest: `hidden_info` (protocol, 1,050 s, one case 272 s), `dk1e_deck` (631 s), `house_commander` (468 s), `free_loops`, `replacement_budget`, `manifest_view`, `foretell_view`, `filter_lands`.
- Every card file lives in `mtg-cards`, and every crate except `mtg-core`, `mtg-import` and `mtg-db` depends on it, so `affected` for one card or engine change selects nearly the whole workspace. A protocol change selects `mtg-protocol`, `mtg-server` and the whole web check.
- Stamps are keyed by the tree hash, so any change anywhere reruns every selected crate.
- Eight agents running the full check at once share 4 test slots of 6 threads; runs queue behind each other and the 40-minute cap kills those that start late.

## Design

- Three levels. The gate (agents, every commit): fmt, clippy of the changed crates, the tests of the changed crates without the slow set, the tests that name a changed card. The batch run (orchestrator, once per merged batch): every test of `affected`, including the slow set. The tag run (before a release): everything with the long fuzz counts.
- The slow set is a committed list of test paths (`scripts/slow-tests.txt`): the gate passes them as `--skip`, the batch and tag runs include them. A gate test over 5 s that is not on the list fails the gate, so the list stays current.
- Stamps are keyed by content: a crate's key hashes its sources, the sources of its workspace dependencies, `Cargo.lock` and the toolchain; a merge reruns only crates whose key changed, and a release of a checked commit skips the check.
- A change only to card files (`crates/mtg-cards/cards/`) tests those cards, the test modules and fixture decks that name them and a fixed sample; the batch run tests the whole corpus.
- Test binaries run side by side, each with a share of the threads; the Postgres crates, the other crates and the web run at once. The thread budget comes from the cores and the checks already running, not from fixed slots.
- The release images build while the batch run of the release commit runs, both at once; `scripts/release.sh` finds them in the build cache and only pushes and rolls out.
- A new worktree starts from a copy of the warm `target` of its base commit instead of a cold build.

## Projection

| Case | Today | After ts1a to ts1e |
| --- | --- | --- |
| Agent gate, engine or card change | 35 to 60 min | 3 to 5 min |
| Orchestrator batch run | 35 to 60 min | about 13 min (the longest binary) |
| Release of a checked commit | about 70 min | about 3 min (push and rollout) |

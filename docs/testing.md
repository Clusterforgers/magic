# Testing

How the checks run, what they test and how a new worktree gets a warm build.

## Commands

| Command | Who | Runs |
| --- | --- | --- |
| `scripts/check.sh gate` | agents, every commit | fmt, clippy of the changed crates, `cargo check` of their dependents, the script tests, the tests of the changed crates without the slow list, the web checks when `web/` changed |
| `scripts/check.sh affected` | orchestrator, once per merged batch | fmt, clippy, the script tests, every test of the changed crates and their dependents (slow list included), the web checks the diff needs |
| `scripts/check.sh all` | orchestrator, before a tag | everything, the whole web check included |
| `scripts/check.sh fast` | commit hook | fmt, clippy, the script tests |

- A diff is taken against the merge-base with `origin/main` (`gate BASE` and `affected BASE` name another); `--list` prints the selection and exits. A change to the check scripts alone selects no crate in the gate and every crate in `affected`.
- The slow list `scripts/slow-tests.txt` names the tests over 5 s (substrings of the test path, one per line, a case table by its function). The gate passes them to the test binaries as `--skip`; `affected` and `all` run them. A test over 5 s that ran and is not on the list is printed with its seconds at the end of the run.
- The test build is one `cargo test --no-run` of the selected crates. Each test binary then runs as its own process from its crate folder with the environment cargo gives a test (`CARGO_MANIFEST_DIR`, `CARGO_PKG_*`, `CARGO_BIN_EXE_*`) and with `--report-time`, so the feature set of the build is the one tested. A crate with doc tests also runs `cargo test --doc`.
- Test binaries and the web checks run side by side within a thread budget: the cores (`MTG_TEST_THREADS`, default `nproc`) divided by the checks running (a check blocked only on the Postgres lock does not count), at least 4. The biggest binary starts first with a third of the free threads, the next with a third of the rest, down to 2 (`RUST_TEST_THREADS` fixes the count); a binary starts when its threads are free. The web checks start first and run while the Rust phases compile.
- The binaries of the Postgres crates (`mtg-db`, `mtg-server`, `mtg-cli`) run one at a time under a machine-wide file lock, because `sqlx::test` drops the test databases of other runs.
- The first failing binary kills the rest. The run ends with the failure section of that binary and the names of the failing tests; a binary over `MTG_TEST_TIMEOUT` minutes (40) is killed and the tests that ran over 60 s are named.
- Every phase result is a stamp in `~/.cache/mtg-check/stamps`, keyed by content: the sources of the crate and of its workspace dependencies, `Cargo.lock`, the toolchain, `.cargo`, `clippy.toml`, the Postgres crates' `.sqlx`, and for the tests of `mtg-protocol` the fixtures and bindings in `web/src/protocol`; the web key hashes `web/`, the lock files and the node version. Docs and unrelated crates do not change a key, so a merge reruns only the crates whose key changed, worktrees with equal content share stamps, and a checked commit needs no new check for a release. The working tree is hashed with a temporary index (tracked and untracked files, not ignored ones); `--force` reruns. A gate pass leaves a `gate-test` stamp that `affected` does not accept.

## Costs

- Every card file lives in `mtg-cards`, and every crate except `mtg-core`, `mtg-import` and `mtg-db` depends on it, so `affected` for one card or engine change selects nearly the whole workspace; the gate tests the changed crate only. A protocol change selects `mtg-protocol`, `mtg-server` and the whole web check.
- `mtg-cards` builds with `opt-level = 3` in the dev profile (like the engine crates): its tests parse the whole card corpus.
- Each test process loads the built-in library once (`OnceLock`, `builtin::library()`, read-only and shared), so one process per test (nextest) pays the load per test; libtest in one process pays it once.
- Server tests look a card id up by name through `common::oracle_id_of`, which parses the corpus once per process.
- A test over 5 s belongs on `scripts/slow-tests.txt`; the long tail of slow and fuzz tests decides the wall clock of `affected`.

## Warm worktrees

`scripts/warm-target.sh` keeps a warm copy of `target` per toolchain and commit in `~/.cache/mtg-cli/warm-target/<toolchain>-<commit>` (about 12 GB each) and starts a new worktree from it.

- `refresh [REF]` (orchestrator, after a green batch) extracts `main` into a clean directory, runs the cargo calls of `scripts/check.sh` there (clippy of every target, then the test builds of the Postgres crates and of the other crates) and stores the `target`. It keeps the newest two entries and names the older ones before removing them. The key is the digest of `rustc -vV`, `rust-toolchain.toml`, `.cargo/config.toml` and `RUSTFLAGS` plus the commit.
- `use` (first command of a new worktree) picks the entry of this toolchain whose commit differs from the checkout in the fewest files and copies it into `target`: reflinks where the filesystem has them, else hardlinks for `.rlib`, `.rmeta` and `.so` outputs (read-only in the cache, so a write in place fails instead of changing every worktree) and a copy for the rest, else a plain copy. Cargo rebuilds a crate whose source is newer than its artifacts, so `use` sets the time of every file and directory equal to the entry's commit to that commit's time and the time of every changed or untracked file to now.
- Cargo bakes the absolute path of the checkout into `mtg-cards` (`include_str!` of every card file, `OUT_DIR`), `mtg-cli`, `mtg-db`, `mtg-server` and `mtg-testkit` (`CARGO_MANIFEST_DIR`) and into everything that depends on them: a copy at another path rebuilds most workspace crates. `refresh` and `run` therefore work in a private user and mount namespace (`unshare -Urm`) where the checkout is bind-mounted at one fixed directory (`<cache>/ws`), and the same copy is up to date there. Without namespaces `use` leaves `target` empty.
- `run -- COMMAND...` runs a command that way. `scripts/warm-bin/cargo` is a `cargo` that does it for a checkout with `target/.warm-target`; with `scripts/warm-bin` first in `PATH` a warm worktree needs no other change. Test binaries of a warm worktree find their fixtures only inside the namespace.
- Cargo unifies dependency features per package selection, so a warm `target` serves the selections it was built for: clippy of the workspace, all other crates together and the Postgres crates together. Another selection (one crate) builds its dependencies again, as in a cold worktree. `MTG_WARM_BUILD` replaces the build command.
- `use` refuses a `target` that is not empty (`--replace` discards the worktree's own `target` only) and a symlinked one, and `refresh` removes only entries of its own cache.

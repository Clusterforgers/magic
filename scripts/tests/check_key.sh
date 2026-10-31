#!/usr/bin/env bash
# Goal: a stamp key hashes the content that decides a phase: a crate's sources, its workspace dependencies' sources and
# the root config; a docs change or an unrelated crate leaves it unchanged; a dirty tree keys like the commit of it.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=probe.bash
source scripts/tests/probe.bash
# shellcheck source=../lib/check-plan.sh
source scripts/lib/check-plan.sh
load_graph

# changed_crates <kind> <commit>: the crates whose key differs between HEAD and the commit.
changed_crates() {
  local crate before out=()
  for crate in "${all_crates[@]}"; do
    stamp_line "$head" "$1" "$crate"
    before="$REPLY"
    stamp_line "$2" "$1" "$crate"
    [[ "$before" == "$REPLY" ]] || out+=("$crate")
  done
  ((${#out[@]} == 0)) || printf '%s\n' "${out[@]}" | sort | tr '\n' ' ' | sed 's/ $//'
}

# case_key <name> <kind> <crates whose key changes> -- <changed path...>
case_key() {
  local name="$1" kind="$2" want="$3"
  shift 4
  check "$name ($kind)" "$(changed_crates "$kind" "$(probe_commit "$@")")" "$want"
}

all="mtg-cards mtg-cli mtg-core mtg-db mtg-engine mtg-import mtg-protocol mtg-server mtg-testkit"
pg="mtg-cli mtg-db mtg-server"
for kind in test clippy check; do
  case_key "docs only" $kind "" -- docs/ROADMAP.md CLAUDE.md
  case_key "scripts" $kind "" -- scripts/agent-status.sh scripts/check.sh
  case_key "web source" $kind "" -- web/src/lib/flash.ts
  case_key "leaf crate" $kind "mtg-protocol mtg-server" -- crates/mtg-protocol/src/lib.rs
  case_key "dependency of all" $kind "$all" -- crates/mtg-core/src/lib.rs
  case_key "a test file" $kind "mtg-cli mtg-protocol mtg-server mtg-testkit" -- crates/mtg-testkit/tests/it/new.rs
  case_key "Cargo.lock" $kind "$all" -- Cargo.lock
  case_key "toolchain file" $kind "$all" -- rust-toolchain.toml
  case_key "cargo config" $kind "$all" -- .cargo/extra.toml
  case_key "sqlx metadata" $kind "$pg" -- .sqlx/query-probe.json
  case_key "database crate" $kind "$pg" -- crates/mtg-db/src/lib.rs
  case_key "protocol fixture" $kind "mtg-protocol" -- web/src/protocol/fixtures/probe.json
done
# The slow list decides what the gate runs, nothing else.
case_key "slow list" test "" -- scripts/slow-tests.txt
case_key "slow list" gate-test "$all" -- scripts/slow-tests.txt

# The key names the phase, the crate and the tool as well.
stamp_line "$head" test mtg-core
a="$REPLY"
stamp_line "$head" clippy mtg-core
b="${REPLY#* }"
stamp_line "$head" test mtg-engine
check "kinds differ" "$([[ "$a" != "clippy:mtg-core $b" ]] && echo differ)" differ
check "crates differ" "$([[ "${a#* }" != "${REPLY#* }" ]] && echo differ)" differ

# The web key follows the web sources and its lock files only.
web_changed() {
  local before
  stamp_line "$head" web -
  before="$REPLY"
  stamp_line "$(probe_commit "$@")" web -
  [[ "$before" == "$REPLY" ]] && echo same || echo changed
}
check "web: docs" "$(web_changed docs/ROADMAP.md)" same
check "web: a crate" "$(web_changed crates/mtg-core/src/lib.rs)" same
check "web: source" "$(web_changed web/src/lib/flash.ts)" changed
check "web: lock file" "$(web_changed pnpm-lock.yaml)" changed

# A dirty tree keys like the commit of the same content, ignored files stay out and the real index stays as it was.
repo="$scratch/repo"
mkdir "$repo"
# A commit hook runs this with GIT_DIR and GIT_INDEX_FILE set; the scratch repository must not see any of them.
for var in $(compgen -e | grep '^GIT_' || true); do unset "$var"; done
export GIT_AUTHOR_NAME=probe GIT_AUTHOR_EMAIL=probe@localhost GIT_COMMITTER_NAME=probe GIT_COMMITTER_EMAIL=probe@localhost
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.hooksPath GIT_CONFIG_VALUE_0=/dev/null
cd "$repo"
git init -q
[[ "$(git rev-parse --show-toplevel)" == "$(realpath "$repo")" ]] || { echo "check_key: not in the scratch repository" >&2 && exit 1; }
mkdir -p crates/a
echo one >crates/a/x.rs
echo "*.log" >.gitignore
git add -A
git commit -qm one
check "clean snapshot" "$(snapshot_tree)" "$(git rev-parse 'HEAD^{tree}')"
echo two >crates/a/x.rs
echo new >crates/a/y.rs
echo junk >crates/a/z.log
dirty=$(snapshot_tree)
check "index untouched" "$(git status --porcelain | tr '\n' '|')" " M crates/a/x.rs|?? crates/a/y.rs|"
git add -A
git commit -qm two
check "dirty snapshot" "$dirty" "$(git rev-parse 'HEAD^{tree}')"
echo gone >crates/a/y.rs && rm crates/a/x.rs
check "deleted file" "$([[ "$(snapshot_tree)" != "$dirty" ]] && echo differs)" differs
finish check_key

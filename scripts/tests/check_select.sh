#!/usr/bin/env bash
# Goal: `scripts/check.sh affected --list` picks the phases a diff can break, and a passed-tree stamp skips a crate.
# Each case commits a probe change on top of HEAD into a throwaway object store and lists the diff of the two commits.
set -euo pipefail
cd "$(dirname "$0")/../.."

scratch=$(mktemp -d "${TMPDIR:-/tmp}/check-select.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
# New objects land in the scratch store; the repository's own objects are read through the alternate.
GIT_ALTERNATE_OBJECT_DIRECTORIES="$(git rev-parse --path-format=absolute --git-common-dir)/objects"
export GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_OBJECT_DIRECTORY="$scratch/objects"
mkdir -p "$GIT_OBJECT_DIRECTORY"
export GIT_AUTHOR_NAME=probe GIT_AUTHOR_EMAIL=probe@localhost GIT_COMMITTER_NAME=probe GIT_COMMITTER_EMAIL=probe@localhost
export MTG_CHECK_STAMPS="$scratch/stamps"

head=$(git rev-parse HEAD)
failures=0 cases=0

# probe_commit <path...>: a commit on top of HEAD that changes (or adds) each path.
probe_commit() {
  local index="$scratch/index" path blob tree
  GIT_INDEX_FILE="$index" git read-tree "$head"
  for path in "$@"; do
    blob=$(echo "probe $path" | git hash-object -w --stdin)
    GIT_INDEX_FILE="$index" git update-index --add --cacheinfo "100644,$blob,$path"
  done
  tree=$(GIT_INDEX_FILE="$index" git write-tree)
  git commit-tree -p "$head" -m probe "$tree"
}

# field <listing> <label>: the sorted words after "<label>: " in a listing.
field() {
  sed -n "s/^$2: //p" <<<"$1" | tr ' ' '\n' | sort | tr '\n' ' ' | sed 's/ $//'
}

# expect <name> <listing> <without Postgres> <with Postgres> <web>
expect() {
  local got_other got_pg got_web
  got_other=$(field "$2" "tests without Postgres")
  got_pg=$(field "$2" "tests with Postgres")
  got_web=$(field "$2" "web")
  cases=$((cases + 1))
  if [[ "$got_other|$got_pg|$got_web" != "$3|$4|$5" ]]; then
    failures=$((failures + 1))
    echo "FAIL $1: got [$got_other|$got_pg|$got_web], want [$3|$4|$5]"
  fi
}

# case_diff <name> <without Postgres> <with Postgres> <web> -- <changed path...>
case_diff() {
  local name="$1" other="$2" pg="$3" web="$4" commit
  shift 5
  commit=$(probe_commit "$@")
  expect "$name" "$(scripts/check.sh affected --list "$head" "$commit")" "$other" "$pg" "$web"
}

all_other="mtg-cards mtg-core mtg-engine mtg-import mtg-protocol mtg-testkit"
all_pg="mtg-cli mtg-db mtg-server"
case_diff "only docs" - - none -- docs/ROADMAP.md CLAUDE.md
case_diff "other scripts" - - none -- scripts/agent-status.sh
case_diff "mtg-cards" "mtg-cards mtg-engine mtg-protocol mtg-testkit" "mtg-cli mtg-server" none -- crates/mtg-cards/src/lib.rs
case_diff "mtg-core" "$all_other" "$all_pg" none -- crates/mtg-core/src/lib.rs
case_diff "only web" - - changed -- web/src/lib/flash.ts
case_diff "web config" - - full -- web/src/lib/flash.ts web/package.json
case_diff "bindings" - - full -- web/src/protocol/generated/Rule.ts
case_diff "mtg-protocol" "mtg-protocol" "mtg-server" full -- crates/mtg-protocol/src/lib.rs
case_diff "Cargo.lock" "$all_other" "$all_pg" full -- Cargo.lock
case_diff "check.sh" "$all_other" "$all_pg" full -- scripts/check.sh
case_diff "mtg-db" - "$all_pg" none -- crates/mtg-db/src/lib.rs
case_diff "migration" - "$all_pg" none -- crates/mtg-db/migrations/9999_probe.sql
case_diff ".sqlx" - "$all_pg" none -- .sqlx/query-probe.json
case_diff "web and mtg-cli" - "mtg-cli" changed -- web/src/lib/flash.ts crates/mtg-cli/src/main.rs

# A stamp for this clean tree skips those crates: the diff runs from a probe base back to the working tree.
if [[ -z "$(git status --porcelain)" ]]; then
  tc=$(rustc -V | tr ' ' '_')
  printf '%s %s test:%s\n' "$(git rev-parse 'HEAD^{tree}')" "$tc" mtg-db "$(git rev-parse 'HEAD^{tree}')" "$tc" mtg-server \
    >"$MTG_CHECK_STAMPS"
  expect "stamped" "$(scripts/check.sh affected --list "$(probe_commit crates/mtg-db/src/lib.rs)")" - mtg-cli none
  expect "forced" "$(scripts/check.sh affected --list --force "$(probe_commit crates/mtg-db/src/lib.rs)")" - "$all_pg" none
else
  echo "check_select: stamp cases skipped (dirty worktree)"
fi

if ((failures)); then
  echo "check_select: $failures of $cases cases failed"
  exit 1
fi
echo "check_select: $cases cases ok"

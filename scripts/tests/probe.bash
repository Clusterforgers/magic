# Shared by the script tests (sourced, not run): case counting, probe commits in a throwaway object store and lock
# holders. A probe commit sits on top of HEAD (or the tree in $probe_tree) and changes or adds the paths it is given.
# Every test runs in a scratch directory of its own: temp files, locks and leases land there, and nothing of the
# caller's environment (the knobs of a running check, the leases and Postgres lock of other checks) reaches it.

scratch=$(mktemp -d "${TMPDIR:-/tmp}/check-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
export TMPDIR="$scratch" XDG_RUNTIME_DIR="$scratch"
unset MTG_TEST_THREADS MTG_TEST_TIMEOUT MTG_PG_SHARED MTG_PG_IMAGE MTG_SEED_SLICE MTG_CHECK_LEASES RUST_TEST_THREADS
# The lease (fd 7) and the Postgres lock (fd 9) of the check that started this test stay with that check.
exec 7>&- 9>&-
# New objects land in the scratch store; the repository's own objects are read through the alternate.
GIT_ALTERNATE_OBJECT_DIRECTORIES="$(git rev-parse --path-format=absolute --git-common-dir)/objects"
export GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_OBJECT_DIRECTORY="$scratch/objects"
mkdir -p "$GIT_OBJECT_DIRECTORY"
export GIT_AUTHOR_NAME=probe GIT_AUTHOR_EMAIL=probe@localhost GIT_COMMITTER_NAME=probe GIT_COMMITTER_EMAIL=probe@localhost
export MTG_CHECK_STAMPS="$scratch/stamps"

head=$(git rev-parse HEAD)
failures=0 cases=0

# probe_commit <path...>: the commit that changes (or adds) each path on top of the probe tree.
probe_commit() {
  local index="$scratch/index" path blob tree
  GIT_INDEX_FILE="$index" git read-tree "${probe_tree:-$head}"
  for path in "$@"; do
    blob=$(echo "probe $path" | git hash-object -w --stdin)
    GIT_INDEX_FILE="$index" git update-index --add --cacheinfo "100644,$blob,$path"
  done
  tree=$(GIT_INDEX_FILE="$index" git write-tree)
  git commit-tree -p "$head" -m probe "$tree"
}

# check <name> <got> <want>: count a case and report a difference.
check() {
  cases=$((cases + 1))
  if [[ "$2" != "$3" ]]; then
    failures=$((failures + 1))
    echo "FAIL $1: got [$2], want [$3]"
  fi
}

# field <listing> <label>: the sorted words after "<label>: " in a listing.
field() {
  sed -n "s/^$2: //p" <<<"$1" | tr ' ' '\n' | sort | tr '\n' ' ' | sed 's/ $//'
}

# hold_lock <file>: hold an exclusive lock on the file in a background process, returning once it is held. The lock stays
# until the file $scratch/release exists (release_lock, or a case that touches it) or a minute has passed, so a test that
# dies leaves nothing behind and no case depends on how long the lock lasts. $scratch/gone appears when it is let go.
hold_lock() {
  local i=0
  rm -f "$scratch/held" "$scratch/release" "$scratch/gone"
  flock "$1" sh -c 'touch "$1"; n=0; while [ -d "$3" ] && [ ! -e "$2" ] && [ $n -lt 1200 ]; do sleep 0.05; n=$((n + 1)); done; touch "$4" 2>/dev/null' \
    sh "$scratch/held" "$scratch/release" "$scratch" "$scratch/gone" &
  holder=$!
  until [[ -e "$scratch/held" ]] || ((i++ >= 1000)); do sleep 0.02; done
}

# release_lock: let go of the lock hold_lock holds and wait for its process.
release_lock() {
  touch "$scratch/release"
  wait "$holder" 2>/dev/null || true
}

# finish <suite>: print the result and set the exit status.
finish() {
  if ((failures)); then
    echo "$1: $failures of $cases cases failed"
    exit 1
  fi
  echo "$1: $cases cases ok"
}

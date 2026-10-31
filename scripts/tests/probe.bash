# Shared by the script tests (sourced, not run): case counting and probe commits in a throwaway object store.
# A probe commit sits on top of HEAD (or the tree in $probe_tree) and changes or adds the paths it is given.

scratch=$(mktemp -d "${TMPDIR:-/tmp}/check-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
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

# finish <suite>: print the result and set the exit status.
finish() {
  if ((failures)); then
    echo "$1: $failures of $cases cases failed"
    exit 1
  fi
  echo "$1: $cases cases ok"
}

#!/usr/bin/env bash
# shellcheck disable=SC2034,SC2154 # the libraries share globals with check.sh
# Plan helpers of scripts/check.sh: crate graph, selection by diff, content keys, stamps, slow-test list, thread budget.
# Sourced by check.sh and the script tests; defines functions and tables only.

# Only these crates connect to Postgres (`sqlx::test`); the others run without the database or the lock.
pg_crates=" mtg-db mtg-server mtg-cli "
# Inputs a crate reads outside its own folder and its workspace dependencies.
declare -A extra_inputs=([mtg-protocol]="web/src/protocol")
declare -A crate_dir=() crate_deps=()
all_crates=()
slow_file="scripts/slow-tests.txt"
slow_limit=5
slice_file="scripts/seed-slices.txt"
gate_slices=4
# A seeded suite on the slice list may hold tests up to this many seconds; the gate runs a quarter of them.
slice_limit=20

# --- Crate graph -------------------------------------------------------------------------------------------------

# Lines "<crate> <dir relative to the root> <workspace deps, comma-separated>" from `cargo metadata`.
crate_graph() {
  cargo metadata --no-deps --format-version 1 --offline |
    jq -r '.workspace_root as $root | [.packages[].name] as $members | .packages[]
      | [.name, (.manifest_path | ltrimstr($root + "/") | rtrimstr("/Cargo.toml")),
         ([.dependencies[].name | select(IN($members[]))] | unique | join(","))] | join(" ")'
}

load_graph() {
  local name dir deps
  while read -r name dir deps; do
    crate_dir[$name]="$dir"
    crate_deps[$name]="$deps"
    all_crates+=("$name")
  done < <(crate_graph)
}

# --- Selection ---------------------------------------------------------------------------------------------------

declare -A hit=() input_hit=()
select_all=0 web_sel=none gate=0

# classify <path>: mark what a changed file can break. The gate leaves the check scripts to the script tests.
classify() {
  local f="$1" name
  # A crate whose tests read the file (extra_inputs) is tested; its dependents do not read it.
  for name in "${!extra_inputs[@]}"; do
    [[ "$f" != "${extra_inputs[$name]}"/* ]] || input_hit[$name]=1
  done
  case "$f" in
    scripts/check.sh | scripts/lib/check-*.sh) ((gate)) || select_all=1 ;;
    Cargo.toml | Cargo.lock | rust-toolchain | rust-toolchain.toml | .cargo/*) select_all=1 ;;
    .sqlx/* | docker-compose.yml) hit[mtg-db]=1 ;;
    package.json | pnpm-lock.yaml | pnpm-workspace.yaml | web/src/protocol/generated/* | web/package.json | \
      web/pnpm-lock.yaml | web/vite.config.ts | web/tsconfig*.json | web/src/test-setup.ts) web_sel=full ;;
    web/*) [[ "$web_sel" == full ]] || web_sel=changed ;;
    crates/*)
      for name in "${all_crates[@]}"; do
        [[ "$f" == "${crate_dir[$name]}"/* ]] && hit[$name]=1 && return
      done
      select_all=1
      ;;
  esac
}

# close_over_dependents: add every crate that depends on a selected crate, until nothing changes.
close_over_dependents() {
  local grew=1 name dep
  while ((grew)); do
    grew=0
    for name in "${all_crates[@]}"; do
      [[ -v hit[$name] ]] && continue
      for dep in ${crate_deps[$name]//,/ }; do
        if [[ -v hit[$dep] ]]; then
          hit[$name]=1 grew=1
          break
        fi
      done
    done
  done
}

# closure <crate>: the crate and its workspace dependencies, transitively, one per line.
closure() {
  local -A seen=()
  local todo=("$1") name dep
  while ((${#todo[@]})); do
    name="${todo[-1]}"
    unset 'todo[-1]'
    [[ -v seen[$name] ]] && continue
    seen[$name]=1
    for dep in ${crate_deps[$name]//,/ }; do todo+=("$dep"); done
  done
  printf '%s\n' "${!seen[@]}" | sort
}

# --- Content keys and stamps -------------------------------------------------------------------------------------

stamp_file="${MTG_CHECK_STAMPS:-${XDG_CACHE_HOME:-$HOME/.cache}/mtg-check/stamps}"
declare -A tree_cache=()
rust_tc="" node_v=""

# snapshot_tree: the git tree of the working directory (tracked and untracked files, not ignored ones), clean or not.
snapshot_tree() {
  local idx rc=0
  if [[ -z "$(git status --porcelain)" ]]; then
    git rev-parse 'HEAD^{tree}'
    return
  fi
  idx=$(mktemp "${TMPDIR:-/tmp}/mtg-check-index.XXXXXX")
  cp "${GIT_INDEX_FILE:-$(git rev-parse --path-format=absolute --git-path index)}" "$idx"
  GIT_INDEX_FILE="$idx" git add -A >/dev/null 2>&1 && GIT_INDEX_FILE="$idx" git write-tree || rc=1
  rm -f "$idx"
  return "$rc"
}

# tree_at <rev> <path>: the git object id of a path in a commit or tree, "-" when it is missing (sets REPLY).
tree_at() {
  local k="$1:$2"
  if [[ ! -v tree_cache[$k] ]]; then
    tree_cache[$k]=$(git rev-parse --verify --quiet "$k" 2>/dev/null || echo -)
  fi
  REPLY="${tree_cache[$k]}"
}

# key_paths <kind> <crate>: the paths whose content decides a phase result.
key_paths() {
  local kind="$1" name="${2:-}" dep
  case "$kind" in
    web*) printf '%s\n' web package.json pnpm-lock.yaml pnpm-workspace.yaml ;;
    *)
      printf '%s\n' Cargo.toml Cargo.lock rust-toolchain.toml clippy.toml .cargo
      for dep in $(closure "$name"); do echo "${crate_dir[$dep]}"; done
      [[ -z "${extra_inputs[$name]:-}" ]] || echo "${extra_inputs[$name]}"
      [[ "$pg_crates" != *" $name "* ]] || printf '%s\n' .sqlx docker-compose.yml
      [[ "$kind" != gate-test ]] || printf '%s\n' "$slow_file" "$slice_file"
      ;;
  esac
}

# tool_id <kind>: the version of the tool that runs the phase (sets REPLY).
tool_id() {
  case "$1" in
    web*) node_v="${node_v:-$(node -v 2>/dev/null || echo none)}" && REPLY="node-$node_v" ;;
    *) rust_tc="${rust_tc:-$(rustc -V | tr ' ' '_')}" && REPLY="$rust_tc" ;;
  esac
}

# stamp_line <rev> <kind> <name>: sets REPLY to "<kind>:<name> <key>", the key hashing the content that decides the
# phase. It runs in the calling shell so that the git and tool lookups stay cached.
stamp_line() {
  local rev="$1" kind="$2" name="$3" data path
  tool_id "$kind"
  data="$kind $name $REPLY"$'\n'
  for path in $(key_paths "$kind" "$name"); do
    tree_at "$rev" "$path"
    data+="$path $REPLY"$'\n'
  done
  REPLY="$kind:$name $(sha256sum <<<"$data" | cut -c1-40)"
}

# --- Slow tests --------------------------------------------------------------------------------------------------

# slow_entries: the names in the slow-test list (substrings of test paths), without comments and blank lines.
slow_entries() {
  sed -e 's/#.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' "$slow_file" | grep -v '^$' || true
}

# slow_skip_args: the `--skip <name>` arguments of a test binary for the slow list, one word per line.
slow_skip_args() {
  slow_entries | awk '{ print "--skip"; print $0 }'
}

# --- Seed slices -------------------------------------------------------------------------------------------------

# seed_slice <mode> [commit]: sets REPLY to the seed slice of the run, "k/n" (1 <= k <= n) or "all". MTG_SEED_SLICE
# names it; else the gate takes slice (commit mod gate_slices) + 1 of the commit (default HEAD), so successive commits
# cover every slice and a rerun on one commit runs the same tests, and every other mode runs all. Fails on a bad value.
seed_slice() {
  local spec="${MTG_SEED_SLICE:-}" rev="${2:-HEAD}" sha k n
  if [[ -z "$spec" ]]; then
    if [[ "$1" != gate ]]; then
      REPLY=all
      return 0
    fi
    sha=$(git rev-parse --verify --quiet "$rev^{commit}" 2>/dev/null || echo 0)
    spec="$((16#${sha:0:7} % gate_slices + 1))/$gate_slices"
  fi
  if [[ "$spec" == all ]]; then
    REPLY=all
    return 0
  fi
  if [[ ! "$spec" =~ ^([0-9]+)/([0-9]+)$ ]]; then
    echo "MTG_SEED_SLICE must be all or k/n with 1 <= k <= n, not: $spec" >&2
    return 1
  fi
  k=$((10#${BASH_REMATCH[1]})) n=$((10#${BASH_REMATCH[2]}))
  if ((k < 1 || k > n)); then
    echo "MTG_SEED_SLICE must be all or k/n with 1 <= k <= n, not: $spec" >&2
    return 1
  fi
  REPLY="$k/$n"
}

# slice_entries: the lines "<unit> [<test path prefix>|!<test path prefix>]" of the slice list, without comments and
# blank lines. A unit alone slices all its tests, a prefix the tests that start with it, and `!` keeps those whole.
slice_entries() {
  [[ -f "$slice_file" ]] || return 0
  sed -e 's/#.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' "$slice_file" | grep -v '^$' || true
}

# sliced_unit <unit>: the unit has tests on the slice list.
sliced_unit() {
  slice_entries | awk -v u="$1" '$1 == u { found = 1 } END { exit !found }'
}

# slice_tests <unit> <k/n> <skip substrings, one per line>: of the test names on stdin, those this slice runs: a name
# a skip substring matches is left out, and a name the slice list gives the unit is kept when a hash of the name
# falls in slice k of n. Each case of a table hashes on its own, so a slice holds about 1/n of every listed suite.
slice_tests() {
  local k="${2%/*}" n="${2#*/}"
  awk -v unit="$1" -v k="$k" -v n="$n" -v skips="$3" -v entries="$(slice_entries)" '
    BEGIN {
      ns = split(skips, s, "\n")
      ne = split(entries, line, "\n")
      np = nx = 0
      for (i = 1; i <= ne; i++) {
        m = split(line[i], w, /[[:space:]]+/)
        if (w[1] != unit) continue
        if (m > 1 && substr(w[2], 1, 1) == "!") x[++nx] = substr(w[2], 2)
        else p[++np] = (m > 1 ? w[2] : "")
      }
      for (c = 32; c < 127; c++) ord[sprintf("%c", c)] = c
    }
    {
      for (i = 1; i <= ns; i++) if (s[i] != "" && index($0, s[i])) next
      sliced = 0
      for (i = 1; i <= np; i++) if (index($0, p[i]) == 1) sliced = 1
      for (i = 1; i <= nx; i++) if (index($0, x[i]) == 1) sliced = 0
      if (sliced) {
        h = 0
        for (j = 1; j <= length($0); j++) h = (h * 31 + ord[substr($0, j, 1)]) % 1000003
        if (h % n != k - 1) next
      }
      print
    }'
}

# --- Thread budget -----------------------------------------------------------------------------------------------

# thread_share <cores> <checks running>: the threads one check may use at once, at least 4.
thread_share() {
  local runs=$(($2 > 0 ? $2 : 1)) share
  share=$(($1 / runs))
  echo $((share < 4 ? 4 : share))
}

# unit_threads <free threads> <binaries waiting>: the test threads of the next binary, a third of the free ones while
# three or more wait (the biggest binary comes first and takes the most), between 2 and 12.
unit_threads() {
  local t=$(($1 / ($2 < 3 ? ($2 > 0 ? $2 : 1) : 3)))
  echo $((t < 2 ? 2 : t > 12 ? 12 : t))
}

# expected_threads <free threads> <seconds of the binary> <seconds of every binary waiting>: the test threads of a
# binary whose last run is known, its share of the waiting work (rounded up), between 2 and 12. The binary that took
# longest takes the most, so it does not run on a few threads after the others are done.
expected_threads() {
  local t=$((($1 * $2 + $3 - 1) / ($3 > 0 ? $3 : 1)))
  echo $((t < 2 ? 2 : t > 12 ? 12 : t))
}

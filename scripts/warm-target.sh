#!/usr/bin/env bash
# Keeps a warm copy of the workspace `target` per toolchain and base commit, and starts a new worktree from it.
#   scripts/warm-target.sh key [REF]         print the cache key of REF (default HEAD)
#   scripts/warm-target.sh list              print the cache entries, newest first
#   scripts/warm-target.sh use [--replace] [--dry-run] [--method M]
#                                            copy the nearest warm entry into ./target (M: auto, reflink, hardlink, copy)
#   scripts/warm-target.sh refresh [REF] [--force]
#                                            build REF (default main) in a clean temporary tree and store its target
#   scripts/warm-target.sh run [--] COMMAND...
#                                            run COMMAND with this checkout mounted at the canonical path
# Cargo bakes absolute paths into the workspace crates, so a warm target is only reused by a build that sees the same
# path: `refresh` builds and `run` runs inside a private mount namespace where the checkout appears at one fixed path.
# `scripts/warm-bin/cargo` does that for every cargo call of a warm checkout. Details: docs/testing.md (Warm worktrees).
# Environment: MTG_WARM_CACHE (cache directory), MTG_WARM_KEEP (entries kept, 2), MTG_WARM_METHOD (copy method),
# MTG_WARM_BUILD (the build command of `refresh`).
set -euo pipefail

die() {
  echo "warm-target: $*" >&2
  exit 1
}

cache_root() { echo "${MTG_WARM_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/mtg-cli/warm-target}"; }

# The canonical path: an empty directory every build sees the checkout at.
canon_dir() { echo "$(cache_root)/ws"; }

# canon_ok: whether this kernel lets an unprivileged user enter a user and mount namespace.
canon_ok() { unshare -Urm true 2>/dev/null; }

# ns_run <root> <subdir> <command...>: run a command with <root> bind-mounted at the canonical path, in <subdir> of it.
ns_run() {
  local root=$1 sub=$2 canon
  shift 2
  canon=$(canon_dir)
  mkdir -p "$canon"
  unshare -Urm -- env MTG_WARM_IN_NS=1 bash -c 'mount --bind "$1" "$2" && cd "$2/$3" && shift 3 && exec "$@"' \
    warm-ns "$root" "$canon" "$sub" "$@"
}

# The command `refresh` builds with: the test profile of the workspace and the clippy check of every target.
default_build='cargo test --workspace --no-run --quiet && cargo clippy --workspace --all-targets --quiet'

# Files of a target that rustc writes once under a unique name and replaces by rename, never in place:
# the only ones the hardlink method shares between the cache and the worktrees.
shared_files=(-path '*/deps/*' '(' -name '*.rlib' -o -name '*.rmeta' -o -name '*.so' ')')

repo_root() { git rev-parse --show-toplevel 2>/dev/null || die "not inside a git checkout"; }

# toolchain_hash <root> <ref>: eight hex digits over what makes artifacts of one build unusable for another.
toolchain_hash() {
  {
    (cd "$1" && rustc -vV)
    printf 'RUSTFLAGS=%s\nENCODED=%s\n' "${RUSTFLAGS-}" "${CARGO_ENCODED_RUSTFLAGS-}"
    git -C "$1" show "$2:rust-toolchain.toml" 2>/dev/null || true
    git -C "$1" show "$2:.cargo/config.toml" 2>/dev/null || true
  } | sha256sum | cut -c1-8
}

# commit_of <root> <ref>: the full commit id of a ref.
commit_of() { git -C "$1" rev-parse --verify --quiet "$2^{commit}" || die "unknown ref $2"; }

# key_of <root> <ref>: `<toolchain>-<commit>`, the name of a cache entry.
key_of() {
  local commit
  commit=$(commit_of "$1" "$2")
  echo "$(toolchain_hash "$1" "$commit")-${commit:0:12}"
}

# meta_get <entry> <field>: a field of the entry's `meta` file.
meta_get() { sed -n "s/^$2=//p" "$1/meta" | head -n1; }

# entries <cache>: the complete entries (a `meta` file marks one), newest first, one name per line.
entries() {
  local dir
  [[ -d $1 ]] || return 0
  for dir in "$1"/*/; do
    dir=${dir%/}
    [[ -f $dir/meta && ${dir##*/} =~ ^[0-9a-f]{8}-[0-9a-f]{12}$ ]] || continue
    printf '%s %s\n' "$(meta_get "$dir" created)" "${dir##*/}"
  done | sort -rn | cut -d' ' -f2
}

# pick_entry <cache> <root>: the entry of this toolchain whose commit differs from the checkout in the fewest files.
pick_entry() {
  local cache=$1 root=$2 tc canon name commit count best='' best_count=-1
  tc=$(toolchain_hash "$root" HEAD)
  canon=$(canon_dir)
  while IFS= read -r name; do
    [[ $name == "$tc"-* && $(meta_get "$cache/$name" canon) == "$canon" ]] || continue
    commit=$(meta_get "$cache/$name" commit)
    git -C "$root" cat-file -e "$commit^{commit}" 2>/dev/null || continue
    count=$(git -C "$root" diff --name-only "$commit" | wc -l)
    if ((best_count < 0 || count < best_count)); then
      best=$name best_count=$count
    fi
  done < <(entries "$cache")
  [[ -n $best ]] && echo "$best $best_count"
  return 0
}

# probe_method <entry> <parent>: the best way to copy from the entry into a directory of <parent>'s filesystem.
probe_method() {
  local probe="$1/meta" tmp="$2/.warm-probe-$$"
  if cp --reflink=always "$probe" "$tmp" 2>/dev/null; then
    rm -f "$tmp"
    echo reflink
    return
  fi
  rm -f "$tmp"
  if ln "$probe" "$tmp" 2>/dev/null; then
    rm -f "$tmp"
    echo hardlink
    return
  fi
  echo copy
}

# copy_tree <method> <src> <dst>: copy a target tree; <dst> is new and absolute.
copy_tree() {
  local method=$1 src=$2 dst=$3
  case $method in
    reflink) cp -a --reflink=always -T "$src" "$dst" ;;
    copy) cp -a --reflink=never -T "$src" "$dst" ;;
    hardlink)
      mkdir -p "$dst"
      (cd "$src" && find . -type d -print0) | (cd "$dst" && xargs -0 -r mkdir -p --)
      (cd "$src" && find . -type f '(' "${shared_files[@]}" ')' -print0) |
        (cd "$src" && xargs -0 -r cp -al --parents -t "$dst" --)
      (cd "$src" && find . '(' -type f -o -type l ')' -not '(' "${shared_files[@]}" ')' -print0) |
        (cd "$src" && xargs -0 -r cp -a --parents -t "$dst" --)
      ;;
    *) die "unknown copy method $method (auto, reflink, hardlink, copy)" ;;
  esac
}

# stamp_sources <root> <commit> <ref_time>: cargo rebuilds a crate whose source is newer than its artifacts, so the
# files equal to the entry's commit get the commit's old time and every other file (changed or untracked) the time now.
stamp_sources() {
  local root=$1 commit=$2 ref_time=$3 changed
  changed=$(mktemp "${TMPDIR:-/tmp}/warm-changed.XXXXXX")
  {
    git -C "$root" diff --name-only -z "$commit"
    git -C "$root" ls-files -z --others --exclude-standard
  } >"$changed"
  git -C "$root" ls-files -z | (cd "$root" && xargs -0 -r touch -c -h -d "@$ref_time" --)
  # Cargo reads the time of a `rerun-if-changed` directory too, so directories are old unless an entry came or went.
  (cd "$root" && find . '(' -name target -o -name .git -o -name node_modules ')' -prune -o -type d -print0 |
    xargs -0 -r touch -c -d "@$ref_time" --)
  (cd "$root" && xargs -0 -r touch -c -h -- <"$changed")
  (cd "$root" && xargs -0 -r dirname -z -- <"$changed" | sort -zu | xargs -0 -r touch -c --)
  rm -f -- "$changed"
}

cmd_key() { key_of "$(repo_root)" "${1:-HEAD}"; }

cmd_list() {
  local cache name
  cache=$(cache_root)
  while IFS= read -r name; do
    printf '%s  commit %s  built %s  %s\n' "$name" "$(meta_get "$cache/$name" commit | cut -c1-12)" \
      "$(date -d "@$(meta_get "$cache/$name" created)" '+%F %T')" "$(du -sh "$cache/$name" | cut -f1)"
  done < <(entries "$cache")
}

cmd_use() {
  local replace=0 dry=0 method=${MTG_WARM_METHOD:-auto} root dest cache picked name count entry commit start copied tmp
  while (($#)); do
    case $1 in
      --replace) replace=1 ;;
      --dry-run) dry=1 ;;
      --method) method=${2:?--method needs a value}; shift ;;
      *) die "unknown option $1" ;;
    esac
    shift
  done
  root=$(repo_root)
  dest=$root/target
  cache=$(cache_root)
  if [[ -L $dest ]]; then die "$dest is a symlink; not touching it"; fi
  if [[ -n $(ls -A "$dest" 2>/dev/null) ]] && ((!replace)); then
    die "$dest is not empty; pass --replace to discard it"
  fi
  if ! canon_ok; then
    echo "warm-target: no mount namespace here, so a warm target would not be reused; the first build is cold"
    return 0
  fi
  picked=$(pick_entry "$cache" "$root")
  if [[ -z $picked ]]; then
    echo "warm-target: no warm entry for this toolchain and canonical path; the first build is cold"
    return 0
  fi
  read -r name count <<<"$picked"
  entry=$cache/$name
  [[ $method == auto ]] && method=$(probe_method "$entry" "$root")
  echo "warm-target: $name, files differing from the checkout: $count, method $method"
  ((dry)) && return 0
  commit=$(meta_get "$entry" commit)
  start=$(date +%s)
  exec 9>>"$cache/.lock"
  flock -s 9
  tmp=$root/.warm-target-$$
  trap "rm -rf -- '$tmp'" EXIT
  copy_tree "$method" "$entry/target" "$tmp"
  if [[ -d $dest ]]; then rm -rf -- "$dest"; fi
  mv -T -- "$tmp" "$dest"
  copied=$(date +%s)
  stamp_sources "$root" "$commit" "$(meta_get "$entry" ref_time)"
  printf 'started warm from %s\ncargo must run at the canonical path: scripts/warm-target.sh run -- cargo ...\n' "$name" \
    >"$dest/.warm-target"
  echo "warm-target: ready in $(($(date +%s) - start)) s (copy $((copied - start)) s)"
  echo "warm-target: run cargo as 'scripts/warm-target.sh run -- cargo ...' or put scripts/warm-bin first in PATH"
}

cmd_run() {
  [[ ${1-} != -- ]] || shift
  (($#)) || die "run needs a command"
  if [[ -n ${MTG_WARM_IN_NS-} ]]; then exec "$@"; fi
  if ! canon_ok; then
    echo "warm-target: no mount namespace here; running without the canonical path" >&2
    exec "$@"
  fi
  ns_run "$(repo_root)" "$(git rev-parse --show-prefix)" "$@"
}

# prune <cache>: remove the entries beyond the newest MTG_WARM_KEEP, and leftovers of a crashed refresh.
prune() {
  local cache=$1 keep=${MTG_WARM_KEEP:-2} i leftover names=()
  mapfile -t names < <(entries "$cache")
  for ((i = keep; i < ${#names[@]}; i++)); do
    echo "warm-target: removing ${names[i]}"
    rm -rf -- "${cache:?}/${names[i]}"
  done
  while IFS= read -r leftover; do
    echo "warm-target: removing stale ${leftover##*/}"
    rm -rf -- "$leftover"
  done < <(find "$cache" -mindepth 1 -maxdepth 1 -type d \( -name '.tmp-*' -o -name '.build-*' \) -mmin +1440)
}

cmd_refresh() {
  local ref=main force=0 root cache commit key build tmp start ref_time
  while (($#)); do
    case $1 in
      --force) force=1 ;;
      -*) die "unknown option $1" ;;
      *) ref=$1 ;;
    esac
    shift
  done
  root=$(repo_root)
  cache=$(cache_root)
  commit=$(commit_of "$root" "$ref")
  key=$(toolchain_hash "$root" "$commit")-${commit:0:12}
  mkdir -p "$cache"
  exec 8>>"$cache/.refresh.lock"
  flock -n 8 || die "another refresh is running"
  if [[ -f $cache/$key/meta ]] && ((!force)); then
    echo "warm-target: $key is up to date"
  else
    build=$cache/.build-$key-$$
    tmp=$cache/.tmp-$key-$$
    trap "rm -rf -- '$build' '$tmp'" EXIT
    start=$(date +%s)
    ref_time=$(git -C "$root" show -s --format=%ct "$commit")
    ((ref_time < start)) || ref_time=$((start - 1))
    mkdir -p "$build"
    git -C "$root" archive "$commit" | tar -x -C "$build"
    find "$build" -print0 | xargs -0 -r touch -h -d "@$ref_time" --
    echo "warm-target: building $key"
    canon_ok || die "refresh needs a user and mount namespace (unshare -Urm); this machine has none"
    ns_run "$build" "" env -u CARGO_TARGET_DIR bash -c "${MTG_WARM_BUILD:-$default_build}"
    mkdir -p "$tmp"
    mv -T -- "$build/target" "$tmp/target"
    # In-place writes to a shared file then fail instead of corrupting every worktree.
    (cd "$tmp/target" && find . -type f '(' "${shared_files[@]}" ')' -print0 | xargs -0 -r chmod a-w --)
    printf 'commit=%s\nref_time=%s\ncanon=%s\ncreated=%s\nbuild_seconds=%s\n' "$commit" "$ref_time" "$(canon_dir)" \
      "$(date +%s.%N)" "$(($(date +%s) - start))" >"$tmp/meta"
    exec 9>>"$cache/.lock"
    flock -x 9
    rm -rf -- "${cache:?}/$key"
    mv -T -- "$tmp" "$cache/$key"
    rm -rf -- "$build"
    echo "warm-target: stored $key in $(($(date +%s) - start)) s"
  fi
  exec 9>>"$cache/.lock"
  flock -x 9
  prune "$cache"
}

main() {
  local command=${1:-}
  shift || true
  case $command in
    key) cmd_key "$@" ;;
    list) cmd_list ;;
    use) cmd_use "$@" ;;
    refresh) cmd_refresh "$@" ;;
    run) cmd_run "$@" ;;
    *) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//' && exit 2 ;;
  esac
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then main "$@"; fi

#!/usr/bin/env bash
# Goal: `scripts/warm-target.sh` names its cache keys by toolchain and commit, falls back from reflinks to hardlinks to
# a plain copy, and never deletes or rewrites anything outside the cache directory and the worktree's own `target`.
# Every case runs in a temporary git repository with a stubbed build, so no real cargo build is needed.
set -euo pipefail
script=$(cd "$(dirname "$0")/.." && pwd)/warm-target.sh
real_cp=$(command -v cp)
real_ln=$(command -v ln)
real_unshare=$(command -v unshare || true)
if [[ -z $real_unshare ]] || ! unshare -Urm true 2>/dev/null; then
  echo "warm_target: skipped (no user and mount namespaces here)"
  exit 0
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/warm-target.XXXXXX")
trap 'chmod -R u+w "$scratch" 2>/dev/null || true; rm -rf "$scratch"' EXIT
export HOME="$scratch/home" MTG_WARM_CACHE="$scratch/cache" XDG_CACHE_HOME="$scratch/xdg"
unset CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS MTG_WARM_METHOD MTG_WARM_KEEP
export GIT_AUTHOR_NAME=probe GIT_AUTHOR_EMAIL=probe@localhost GIT_COMMITTER_NAME=probe GIT_COMMITTER_EMAIL=probe@localhost
export MTG_WARM_BUILD='mkdir -p target/debug/deps target/debug/.fingerprint/a target/debug/incremental
  echo rlib >target/debug/deps/liba-1.rlib; echo meta >target/debug/deps/liba-1.rmeta
  echo dep >target/debug/deps/a-1.d; echo fp >target/debug/.fingerprint/a/fp.json; echo inc >target/debug/incremental/state
  pwd -P >target/where'

# Stubs: a fixed compiler version (rustc-version file) and a cp/ln that can pretend the filesystem has no links.
mkdir -p "$scratch/bin"
cat >"$scratch/bin/rustc" <<EOF
#!/bin/sh
echo "rustc \$(cat "$scratch/rustc-version") (stub)"
EOF
cat >"$scratch/bin/cp" <<EOF
#!/usr/bin/env bash
if [[ " \$* " == *" --reflink=always "* ]]; then
  case "\${FAKE_REFLINK-}" in
    1) args=(); for a in "\$@"; do [[ \$a == --reflink=always ]] || args+=("\$a"); done; exec "$real_cp" "\${args[@]}" ;;
    *) exit 1 ;;
  esac
fi
exec "$real_cp" "\$@"
EOF
cat >"$scratch/bin/ln" <<EOF
#!/usr/bin/env bash
[[ -z "\${FAKE_NO_LINK-}" ]] || exit 1
exec "$real_ln" "\$@"
EOF
cat >"$scratch/bin/unshare" <<EOF
#!/usr/bin/env bash
[[ -z "\${FAKE_NO_NS-}" ]] || exit 1
exec "$real_unshare" "\$@"
EOF
mkdir -p "$scratch/real"
cat >"$scratch/real/cargo" <<'EOF'
#!/usr/bin/env bash
echo "real cargo in $(pwd -P): $*"
EOF
chmod +x "$scratch/bin/"* "$scratch/real/"*
export PATH="$scratch/bin:$PATH"
echo 1.0.0 >"$scratch/rustc-version"

failures=0 cases=0
check() { # check <name> <condition exit status>
  cases=$((cases + 1))
  if [[ $2 != 0 ]]; then
    failures=$((failures + 1))
    echo "FAIL $1"
  fi
}
equal() { # equal <name> <got> <want>
  [[ $# -eq 3 ]] || { echo "warm_target: equal called with $# arguments: $1" >&2; exit 2; }
  cases=$((cases + 1))
  if [[ $2 != "$3" ]]; then
    failures=$((failures + 1))
    echo "FAIL $1: got [$2], want [$3]"
  fi
}

repo=$scratch/repo
mkdir -p "$repo/crates/a/src" "$repo/.cargo"
git -C "$repo" init -q -b main
echo '[workspace]' >"$repo/Cargo.toml"
echo 'lock 1' >"$repo/Cargo.lock"
echo 'channel = "stable"' >"$repo/rust-toolchain.toml"
echo '[env]' >"$repo/.cargo/config.toml"
echo 'fn a() {}' >"$repo/crates/a/src/lib.rs"
echo '{}' >"$repo/crates/a/data.json"
commit() { # commit <epoch> <message>
  git -C "$repo" add -A
  GIT_AUTHOR_DATE="@$1 +0000" GIT_COMMITTER_DATE="@$1 +0000" git -C "$repo" commit -q -m "$2"
  git -C "$repo" rev-parse HEAD
}
c1=$(commit 1600000000 one)
echo 'fn a() { 1; }' >"$repo/crates/a/src/lib.rs"
c2=$(commit 1600000100 two)
echo '[env]
X = "1"' >"$repo/.cargo/config.toml"
c3=$(commit 1600000200 three)
warm() { (cd "${WARM_CWD:-$repo}" && "$script" "$@"); }

# Keys: toolchain digest plus commit, stable, and sensitive to exactly the toolchain inputs.
key1=$(warm key "$c1")
key2=$(warm key "$c2")
key3=$(warm key "$c3")
check "key format" "$([[ $key1 =~ ^[0-9a-f]{8}-[0-9a-f]{12}$ ]] && echo 0 || echo 1)"
equal "key is stable" "$(warm key "$c1")" "$key1"
equal "key ends in the commit" "${key1#*-}" "${c1:0:12}"
equal "source change keeps the toolchain part" "${key1%%-*}" "${key2%%-*}"
check "source change changes the key" "$([[ $key1 != "$key2" ]] && echo 0 || echo 1)"
check "cargo config changes the toolchain part" "$([[ ${key2%%-*} != "${key3%%-*}" ]] && echo 0 || echo 1)"
echo 2.0.0 >"$scratch/rustc-version"
check "compiler version changes the toolchain part" "$([[ $(warm key "$c2") != "$key2" ]] && echo 0 || echo 1)"
echo 1.0.0 >"$scratch/rustc-version"
check "RUSTFLAGS change the toolchain part" "$([[ $(RUSTFLAGS=-Cfoo warm key "$c2") != "$key2" ]] && echo 0 || echo 1)"

# Refresh stores the stubbed build, builds in a clean tree and keeps the newest two entries.
mkdir -p "$MTG_WARM_CACHE/keepme"
echo note >"$MTG_WARM_CACHE/notes.txt"
echo 'untracked' >"$repo/scratch.txt"
out=$(warm refresh "$c1" 2>&1)
check "refresh stores an entry" "$([[ -f $MTG_WARM_CACHE/$key1/meta && -f $MTG_WARM_CACHE/$key1/target/debug/deps/liba-1.rlib ]] && echo 0 || echo 1)"
check "refresh says stored" "$([[ $out == *"stored $key1"* ]] && echo 0 || echo 1)"
check "refresh builds a clean tree" "$([[ ! -e $MTG_WARM_CACHE/$key1/target/scratch.txt ]] && echo 0 || echo 1)"
check "refresh leaves no temporary tree" "$([[ -z $(find "$MTG_WARM_CACHE" -maxdepth 1 -name '.build-*' -o -maxdepth 1 -name '.tmp-*') ]] && echo 0 || echo 1)"
check "refresh leaves the checkout alone" "$([[ ! -e $repo/target ]] && echo 0 || echo 1)"
equal "meta holds the commit" "$(sed -n 's/^commit=//p' "$MTG_WARM_CACHE/$key1/meta")" "$c1"
equal "meta holds the canonical path" "$(sed -n 's/^canon=//p' "$MTG_WARM_CACHE/$key1/meta")" "$MTG_WARM_CACHE/ws"
equal "refresh builds at the canonical path" "$(cat "$MTG_WARM_CACHE/$key1/target/where")" "$MTG_WARM_CACHE/ws"
equal "meta holds the commit time" "$(sed -n 's/^ref_time=//p' "$MTG_WARM_CACHE/$key1/meta")" 1600000000
equal "build outputs are shared read-only" "$(stat -c %A "$MTG_WARM_CACHE/$key1/target/debug/deps/liba-1.rlib")" "-r--r--r--"
equal "fingerprints stay writable" "$(stat -c %A "$MTG_WARM_CACHE/$key1/target/debug/.fingerprint/a/fp.json" | cut -c1-4)" "-rw-"
check "a second refresh finds it up to date" "$([[ $(warm refresh "$c1" 2>&1) == *"$key1 is up to date"* ]] && echo 0 || echo 1)"
warm refresh "$c2" >/dev/null 2>&1
out=$(warm refresh "$c3" 2>&1)
check "refresh keeps the newest two" "$([[ ! -e $MTG_WARM_CACHE/$key1 && -f $MTG_WARM_CACHE/$key2/meta && -f $MTG_WARM_CACHE/$key3/meta ]] && echo 0 || echo 1)"
check "refresh names what it removes" "$([[ $out == *"removing $key1"* ]] && echo 0 || echo 1)"
check "refresh removes nothing else" "$([[ -d $MTG_WARM_CACHE/keepme && -f $MTG_WARM_CACHE/notes.txt ]] && echo 0 || echo 1)"
equal "list shows both entries newest first" "$(warm list | cut -d' ' -f1 | tr '\n' ' ')" "$key3 $key2 "
check "a failing build is an error" "$(! (cd "$repo" && MTG_WARM_BUILD='exit 3' "$script" refresh "$c1" --force) >/dev/null 2>&1 && echo 0 || echo 1)"
check "a failing build leaves no entry or tree" "$([[ ! -e $MTG_WARM_CACHE/$key1 && -z $(find "$MTG_WARM_CACHE" -maxdepth 1 -name '.build-*' -o -maxdepth 1 -name '.tmp-*') ]] && echo 0 || echo 1)"
rm -f "$repo/scratch.txt"

# Use: pick the entry that differs in the fewest files, and start cold when the toolchain differs.
wt=$scratch/wt
git -C "$repo" worktree add -q --detach "$wt" "$c1"
equal "dry run names the nearest entry and its distance" "$(WARM_CWD=$wt warm use --dry-run | sed 's/, method.*//')" "warm-target: $key2, files differing from the checkout: 1"
check "dry run copies nothing" "$([[ ! -e $wt/target ]] && echo 0 || echo 1)"
echo 2.0.0 >"$scratch/rustc-version"
equal "another toolchain starts cold" "$(WARM_CWD=$wt warm use 2>&1)" "warm-target: no warm entry for this toolchain and canonical path; the first build is cold"
check "cold start copies nothing" "$([[ ! -e $wt/target ]] && echo 0 || echo 1)"
echo 1.0.0 >"$scratch/rustc-version"

# Use: the copy method falls back from reflink to hardlink to a plain copy.
method_of() { WARM_CWD=$wt warm use --dry-run | sed 's/.*method //'; }
equal "no reflink support falls back to hardlinks" "$(method_of)" hardlink
equal "reflinks win when the filesystem has them" "$(FAKE_REFLINK=1 method_of)" reflink
equal "no links at all falls back to a copy" "$(FAKE_NO_LINK=1 method_of)" copy
equal "a forced method is used" "$(MTG_WARM_METHOD=copy method_of)" copy

# Use: hardlinks share only the build outputs; everything cargo rewrites in place is a private copy.
entry=$MTG_WARM_CACHE/$key2
WARM_CWD=$wt warm use --method hardlink >/dev/null
check "target exists" "$([[ -d $wt/target/debug/deps ]] && echo 0 || echo 1)"
equal "rlib is hardlinked" "$(stat -c %h "$wt/target/debug/deps/liba-1.rlib")" 2
equal "rmeta is hardlinked" "$(stat -c %h "$wt/target/debug/deps/liba-1.rmeta")" 2
equal "dep-info is copied" "$(stat -c %h "$wt/target/debug/deps/a-1.d")" 1
equal "fingerprint is copied" "$(stat -c %h "$wt/target/debug/.fingerprint/a/fp.json")" 1
equal "incremental state is copied" "$(stat -c %h "$wt/target/debug/incremental/state")" 1
equal "content is the same" "$(cat "$wt/target/debug/.fingerprint/a/fp.json")" fp
echo changed >>"$wt/target/debug/.fingerprint/a/fp.json"
equal "a private copy does not touch the entry" "$(cat "$entry/target/debug/.fingerprint/a/fp.json")" fp
check "an in-place write to a shared file is refused" "$(! (echo x >>"$wt/target/debug/deps/liba-1.rlib") 2>/dev/null && echo 0 || echo 1)"
equal "the entry's shared file is unchanged" "$(cat "$entry/target/debug/deps/liba-1.rlib")" rlib
check "a replaced shared file leaves the entry alone" "$(rm -f "$wt/target/debug/deps/liba-1.rlib" && echo new >"$wt/target/debug/deps/liba-1.rlib" && [[ $(cat "$entry/target/debug/deps/liba-1.rlib") == rlib ]] && echo 0 || echo 1)"

# Use: source files equal to the entry's commit are older than its artifacts, changed ones are newer.
equal "an unchanged source has the entry commit time" "$(stat -c %Y "$wt/Cargo.toml")" 1600000100
artifact_time=$(stat -c %Y "$entry/target/debug/deps/a-1.d")
check "a changed source is newer than the artifacts" "$([[ $(stat -c %Y "$wt/crates/a/src/lib.rs") -ge $artifact_time ]] && echo 0 || echo 1)"
echo new >"$wt/untracked.rs"
rm -rf "$wt/target"
WARM_CWD=$wt warm use --method copy >/dev/null
check "an untracked source is newer than the artifacts" "$([[ $(stat -c %Y "$wt/untracked.rs") -ge $artifact_time ]] && echo 0 || echo 1)"
equal "unchanged data stays old after a copy" "$(stat -c %Y "$wt/crates/a/data.json")" 1600000100

# Use: the plain copy shares nothing.
equal "a plain copy shares no inode" "$(stat -c %h "$wt/target/debug/deps/liba-1.rlib")" 1
check "a plain copy keeps the modes" "$([[ $(stat -c %A "$wt/target/debug/deps/liba-1.rlib") == "-r--r--r--" ]] && echo 0 || echo 1)"

# Use: safety rules.
echo mine >"$wt/target/precious"
echo outside >"$scratch/outside"
echo sibling >"$wt/sibling.txt"
check "a filled target is refused" "$(! WARM_CWD=$wt warm use >/dev/null 2>&1 && echo 0 || echo 1)"
equal "a refused use keeps the target" "$(cat "$wt/target/precious")" mine
WARM_CWD=$wt warm use --replace --method copy >/dev/null
check "--replace discards the old target" "$([[ ! -e $wt/target/precious && -d $wt/target/debug/deps ]] && echo 0 || echo 1)"
equal "--replace keeps the worktree's other files" "$(cat "$wt/sibling.txt")" sibling
equal "--replace keeps files outside the worktree" "$(cat "$scratch/outside")" outside
equal "--replace leaves the entry intact" "$(cat "$entry/target/debug/deps/liba-1.rmeta")" meta
rm -rf "$wt/target"
mkdir "$scratch/elsewhere"
echo keep >"$scratch/elsewhere/file"
ln -s "$scratch/elsewhere" "$wt/target"
check "a symlinked target is refused" "$(! WARM_CWD=$wt warm use --replace >/dev/null 2>&1 && echo 0 || echo 1)"
equal "a refused symlink keeps what it points to" "$(cat "$scratch/elsewhere/file")" keep
rm -f "$wt/target"
check "use outside a checkout is refused" "$(! (cd "$scratch" && "$script" use >/dev/null 2>&1) && echo 0 || echo 1)"
check "an unknown command is refused" "$(! "$script" frobnicate >/dev/null 2>&1 && echo 0 || echo 1)"

# Use: the marker tells the shim to run cargo at the canonical path; entries for another path or no namespace are skipped.
WARM_CWD=$wt warm use --method copy >/dev/null
check "use writes the marker" "$([[ -f $wt/target/.warm-target ]] && echo 0 || echo 1)"
sed -i "s|^canon=.*|canon=/elsewhere|" "$MTG_WARM_CACHE/$key2/meta"
check "an entry of another canonical path is skipped" "$([[ $(WARM_CWD=$wt warm use --replace --dry-run 2>&1) == *"no warm entry"* ]] && echo 0 || echo 1)"
sed -i "s|^canon=.*|canon=$MTG_WARM_CACHE/ws|" "$MTG_WARM_CACHE/$key2/meta"
check "without namespaces use starts cold" "$([[ $(FAKE_NO_NS=1 WARM_CWD=$wt warm use --replace --dry-run 2>&1) == *"no mount namespace"* ]] && echo 0 || echo 1)"
check "without namespaces refresh is an error" "$(! (cd "$repo" && FAKE_NO_NS=1 "$script" refresh "$c3" --force) >/dev/null 2>&1 && echo 0 || echo 1)"

# Run: the command sees the checkout at the canonical path, in the same subdirectory, and its exit status comes back.
mkdir -p "$wt/crates/a"
equal "run shows the checkout at the canonical path" "$(cd "$wt" && "$script" run -- pwd -P)" "$MTG_WARM_CACHE/ws"
equal "run keeps the subdirectory" "$(cd "$wt/crates/a" && "$script" run pwd -P)" "$MTG_WARM_CACHE/ws/crates/a"
equal "run gives the command the checkout's files" "$(cd "$wt" && "$script" run -- cat sibling.txt)" sibling
equal "run passes the exit status" "$( (cd "$wt" && "$script" run -- bash -c 'exit 7') >/dev/null 2>&1; echo $?)" 7
equal "run inside a namespace runs in place" "$(cd "$wt" && MTG_WARM_IN_NS=1 "$script" run -- pwd -P)" "$(cd "$wt" && pwd -P)"
equal "run without namespaces runs in place" "$(cd "$wt" && FAKE_NO_NS=1 "$script" run -- pwd -P 2>/dev/null)" "$(cd "$wt" && pwd -P)"
check "run needs a command" "$(! (cd "$wt" && "$script" run) >/dev/null 2>&1 && echo 0 || echo 1)"
check "run outside a checkout is refused" "$(! (cd "$scratch" && "$script" run -- true) >/dev/null 2>&1 && echo 0 || echo 1)"

# Shim: a warm checkout (marker) gets cargo at the canonical path, any other checkout gets the real cargo in place.
shim=$(cd "$(dirname "$script")" && pwd)/warm-bin
cargo_in() { (cd "$1" && PATH="$shim:$scratch/real:$PATH" cargo build -p a); }
equal "the shim runs cargo at the canonical path in a warm checkout" "$(cargo_in "$wt")" "real cargo in $MTG_WARM_CACHE/ws: build -p a"
mv "$wt/target/.warm-target" "$wt/target/.unmarked"
equal "the shim leaves a cold checkout alone" "$(cargo_in "$wt")" "real cargo in $(cd "$wt" && pwd -P): build -p a"
mv "$wt/target/.unmarked" "$wt/target/.warm-target"
equal "the shim runs in place inside a namespace" "$(MTG_WARM_IN_NS=1 cargo_in "$wt")" "real cargo in $(cd "$wt" && pwd -P): build -p a"

# The default build repeats the cargo calls of check.sh: clippy, then the Postgres crates and the others as two test builds.
mkdir -p "$scratch/steps/crates/a" "$scratch/steps/crates/mtg-db" "$scratch/steps/crates/mtg-core" "$scratch/rec"
printf '#!/usr/bin/env bash\necho "$*" >>"%s/calls"\n' "$scratch" >"$scratch/rec/cargo"
chmod +x "$scratch/rec/cargo"
(cd "$scratch/steps" && PATH="$scratch/rec:$PATH" "$script" build-steps)
equal "the default build mirrors check.sh" "$(cat "$scratch/calls")" "clippy --workspace --all-targets --quiet
test -p a -p mtg-core --no-run --quiet
test -p mtg-db --no-run --quiet"

# A refresh prunes only entries of its own cache: other directories survive, even when old.
touch -d '2 days ago' "$MTG_WARM_CACHE/keepme"
MTG_WARM_KEEP=1 warm refresh "$c3" >/dev/null 2>&1
check "keep=1 leaves the newest entry only" "$([[ ! -e $MTG_WARM_CACHE/$key2 && -f $MTG_WARM_CACHE/$key3/meta ]] && echo 0 || echo 1)"
check "unrelated directories survive pruning" "$([[ -d $MTG_WARM_CACHE/keepme && -f $MTG_WARM_CACHE/notes.txt ]] && echo 0 || echo 1)"

if ((failures)); then
  echo "warm_target: $failures of $cases cases failed"
  exit 1
fi
echo "warm_target: $cases cases ok"

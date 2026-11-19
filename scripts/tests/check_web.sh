#!/usr/bin/env bash
# Goal: the fast check runs prettier over exactly the staged web files (not deleted, unstaged or other ones), says which
# files to format and how, and does not block a commit that has no web files or a checkout without prettier.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=probe.bash
source scripts/tests/probe.bash
# shellcheck source=../lib/check-web.sh
source scripts/lib/check-web.sh

# A commit hook runs this with GIT_DIR and GIT_INDEX_FILE set; the scratch repository must not see any of them.
for var in $(compgen -e | grep '^GIT_' || true); do unset "$var"; done
export GIT_AUTHOR_NAME=probe GIT_AUTHOR_EMAIL=probe@localhost GIT_COMMITTER_NAME=probe GIT_COMMITTER_EMAIL=probe@localhost
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.hooksPath GIT_CONFIG_VALUE_0=/dev/null

# Stubs: a prettier that logs its arguments and answers with the exit code and listing the case sets, and a node that
# only has to exist.
repo="$scratch/repo"
mkdir -p "$repo/web/node_modules/.bin" "$repo/web/src" "$repo/crates/a" "$scratch/bin"
printf '#!/bin/sh\n' >"$scratch/bin/node"
cat >"$repo/web/node_modules/.bin/prettier" <<EOF
#!/bin/sh
echo "\$(basename "\$PWD") \$*" >>"$scratch/prettier.log"
cat "$scratch/prettier.out"
exit "\$(cat "$scratch/prettier.rc")"
EOF
chmod +x "$scratch/bin/node" "$repo/web/node_modules/.bin/prettier"
export PATH="$scratch/bin:$PATH"

# staged: run the check; sets rc, out (what it printed) and args (what prettier was called with, "-" for not called).
staged() {
  : >"$scratch/prettier.log"
  rc=0
  out=$(staged_web_format 2>&1) || rc=$?
  args=$(tr ' \n' '\n\n' <"$scratch/prettier.log" | LC_ALL=C sort | tr '\n' ' ')
  [[ -n "$args" ]] || args=-
}
answer() { printf '%s' "$1" >"$scratch/prettier.out" && echo "$2" >"$scratch/prettier.rc"; }

cd "$repo"
git init -q
echo 'node_modules' >.gitignore
echo old >web/src/old.ts
echo keep >web/src/keep.ts
echo loose >web/src/loose.ts
echo lib >crates/a/lib.rs
git add -A
git commit -qm base

# Nothing staged, or a crate file only: prettier does not run.
answer "" 0
staged
check "nothing staged" "$rc $args" "0 -"
echo more >>crates/a/lib.rs
git add crates/a/lib.rs
staged
check "crate file staged" "$rc $args" "0 -"

# Staged web files go to prettier with paths relative to web/: added, modified and renamed ones, not the deleted file,
# and not the file that is changed but not staged.
echo new >web/src/new.ts
echo css >web/src/new.css
echo changed >>web/src/keep.ts
echo changed >>web/src/loose.ts
git rm -q web/src/old.ts
git add web/src/new.ts web/src/new.css web/src/keep.ts
git mv web/src/new.css web/src/renamed.css
staged
check "staged web files" "$rc $args" "0 --ignore-unknown --list-different src/keep.ts src/new.ts src/renamed.css web "

# A listed file fails the check, names the file and the command that formats it, and not the files that are fine.
answer $'src/keep.ts\nsrc/renamed.css\n' 1
staged
check "unformatted: exit code" "$rc" 1
check "unformatted: names the files" "$(grep -c '^  web/src/\(keep.ts\|renamed.css\)$' <<<"$out")" 2
check "unformatted: names the fix" "$(grep -c '^fix: pnpm --dir web exec prettier --write src/keep.ts src/renamed.css (then stage' <<<"$out")" 1
check "unformatted: not the formatted file" "$(grep -c 'new.ts' <<<"$out")" 0

# A prettier that fails without naming a file (a syntax error, a bad config) fails the check too.
answer "" 2
staged
check "prettier error" "$rc $(grep -c 'prettier failed on the staged web files (exit 2' <<<"$out")" "1 1"

# Without prettier in this checkout the check says so and passes.
mv web/node_modules/.bin/prettier "$scratch/prettier.away"
staged
check "no prettier" "$rc $(grep -c 'not checked, prettier is not installed' <<<"$out") $args" "0 1 -"
mv "$scratch/prettier.away" web/node_modules/.bin/prettier

cd - >/dev/null
finish check_web

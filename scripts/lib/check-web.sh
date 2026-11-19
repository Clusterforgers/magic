#!/usr/bin/env bash
# Web format check of the staged files for scripts/check.sh: the fast check's counterpart of `cargo fmt --check`.
# Sourced by check.sh and the script tests; defines functions only.

# staged_web_format: `prettier --list-different` over the staged files of web/ (the paths alone, so the cost follows the
# commit, not the tree); fails with the command that fixes the files it names. Without prettier or node (no
# `pnpm install` in this checkout) it says so and passes: the web checks of the gate run prettier over everything.
staged_web_format() {
  local files=() out rc=0 f
  while IFS= read -r -d '' f; do files+=("${f#web/}"); done < <(git diff --cached --name-only -z --diff-filter=ACMR -- web)
  ((${#files[@]})) || return 0
  if [[ ! -x web/node_modules/.bin/prettier ]] || ! command -v node >/dev/null; then
    echo "web format: not checked, prettier is not installed (pnpm --dir web install)" >&2
    return 0
  fi
  out=$(cd web && node_modules/.bin/prettier --list-different --ignore-unknown "${files[@]}") || rc=$?
  ((rc)) || return 0
  if [[ -z "$out" ]]; then
    echo "web format: prettier failed on the staged web files (exit $rc, see above)" >&2
    return 1
  fi
  echo "web format: staged web files that are not formatted:" >&2
  sed 's|^|  web/|' <<<"$out" >&2
  echo "fix: pnpm --dir web exec prettier --write $(tr '\n' ' ' <<<"$out")(then stage them again)" >&2
  return 1
}

#!/usr/bin/env bash
# One-time local setup: enable the repo's git hooks.
set -euo pipefail
cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
echo "git hooks enabled (.githooks)"

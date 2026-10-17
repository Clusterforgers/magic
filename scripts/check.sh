#!/usr/bin/env bash
# Usage: scripts/check.sh [fast|rust|web|all]   (default: all)
#   fast: fmt + clippy   rust: fast + cargo tests   web: web checks only   all: everything
set -euo pipefail
cd "$(dirname "$0")/.."

mode="${1:-all}"

if [[ "$mode" != web ]]; then
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --quiet -- -D warnings
fi

if [[ "$mode" == rust || "$mode" == all ]]; then
  if command -v docker >/dev/null && [[ -f docker-compose.yml ]]; then
    docker compose up -d --wait db >/dev/null
  fi
  export DATABASE_URL="${DATABASE_URL:-postgres://magic:magic@localhost:5432/magic}"
  cargo test --workspace --quiet
fi

if [[ "$mode" == web || "$mode" == all ]] && [[ -d web ]]; then
  (cd web && pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format && pnpm --silent test)
fi

echo "check ($mode): ok"

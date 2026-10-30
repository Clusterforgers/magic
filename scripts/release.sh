#!/usr/bin/env bash
# Release to the k3s cluster: build both images, push them to the cluster registry, point the cluster
# manifests at the new tag and wait for the rollout. ArgoCD does the deploy (docs/deploy.md).
#
#   scripts/release.sh                   the next patch tag after the latest vX.Y.Z
#   scripts/release.sh v0.2.0            a chosen tag
#   scripts/release.sh --rollback v0.1.3 point the cluster at a tag that is already in the registry
#
# A release refuses while a game is running (the restart cuts its sockets); --even-if-games overrides.
#
# Settings (environment): MTG_REGISTRY (default 100.96.184.94:30500, a Tailscale address),
# MTG_MANIFESTS (default ~/private/Clusterforgers/k3s-cluster-manifests, a clone of the manifests repo).
set -euo pipefail
cd "$(dirname "$0")/.."

REGISTRY="${MTG_REGISTRY:-100.96.184.94:30500}"
MANIFESTS="${MTG_MANIFESTS:-$HOME/private/Clusterforgers/k3s-cluster-manifests}"
VALUES="apps/magic/values.yaml"
BUILDER="magic-release"
APP="magic"
EVEN_IF_GAMES=0

die() { echo "release: $*" >&2; exit 1; }

registry_has() { # image tag
  curl -fsS -m 10 "http://$REGISTRY/v2/$1/tags/list" 2>/dev/null | grep -q "\"$2\""
}

check_tools() {
  for tool in docker git curl; do command -v "$tool" >/dev/null || die "$tool is not installed"; done
  curl -fsS -m 10 "http://$REGISTRY/v2/" >/dev/null || die "the registry $REGISTRY is not reachable (is Tailscale up?)"
  [ -d "$MANIFESTS/.git" ] || die "no manifests clone at $MANIFESTS (set MTG_MANIFESTS)"
  [ -z "$(git -C "$MANIFESTS" status --porcelain)" ] || die "the manifests clone has uncommitted changes"
}

next_tag() {
  local latest
  latest="$(git tag --list 'v[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname | head -1)"
  if [ -z "$latest" ]; then echo v0.1.0; return; fi
  local major minor patch
  IFS=. read -r major minor patch <<<"${latest#v}"
  echo "v$major.$minor.$((patch + 1))"
}

check_source() { # tag
  [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || die "release from main"
  [ -z "$(git status --porcelain)" ] || die "the working tree has uncommitted changes"
  git fetch -q origin main
  [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "HEAD is not origin/main: push or pull first"
  ! git rev-parse -q --verify "refs/tags/$1" >/dev/null || die "the tag $1 exists"
  ! registry_has magic-server "$1" || die "the registry already has $1"
}

ensure_builder() {
  docker buildx inspect "$BUILDER" >/dev/null 2>&1 && return
  local config
  config="$(mktemp)"
  printf '[registry."%s"]\n  http = true\n  insecure = true\n' "$REGISTRY" >"$config"
  docker buildx create --name "$BUILDER" --driver docker-container --driver-opt network=host \
    --buildkitd-config "$config" >/dev/null
  rm -f "$config"
}

build_and_push() { # tag
  local name
  for name in server web; do
    echo "release: building magic-$name:$1"
    docker buildx build --builder "$BUILDER" --platform linux/amd64 -f "deploy/$name.Dockerfile" \
      -t "$REGISTRY/magic-$name:$1" --push .
  done
}

check_games() { # a restart cuts the sockets of a running game, and a rules change can fail it at boot
  [ "$EVEN_IF_GAMES" = 1 ] && return 0
  command -v kubectl >/dev/null || return 0
  local running
  running="$(kubectl -n "$APP" exec magic-db-0 -- psql -U magic -d magic -Atc \
    "SELECT count(*) FROM games WHERE status = 'running'" 2>/dev/null)" || return 0
  [ "${running:-0}" = 0 ] || die "$running game(s) are running: release when they end, or pass --even-if-games"
}

point_manifests() { # tag
  git -C "$MANIFESTS" pull -q --ff-only
  sed -i -E "s|^tag: .*|tag: $1|" "$MANIFESTS/$VALUES"
  grep -q "^tag: $1\$" "$MANIFESTS/$VALUES" || die "could not set the tag in $VALUES"
  if [ -n "$(git -C "$MANIFESTS" status --porcelain -- "$VALUES")" ]; then
    git -C "$MANIFESTS" add "$VALUES"
    git -C "$MANIFESTS" commit -q -m "chore: release $APP $1"
  fi
  git -C "$MANIFESTS" push -q
}

rollout() { # tag
  command -v kubectl >/dev/null || { echo "release: no kubectl; ArgoCD syncs within a few minutes"; return; }
  # The root app finds a new Application, the app itself a new tag; both are asked to look now.
  local app
  for app in apps "$APP"; do
    kubectl -n argocd annotate application "$app" argocd.argoproj.io/refresh=hard --overwrite >/dev/null 2>&1 || true
  done
  local want="$REGISTRY/magic-server:$1" have="" _
  echo "release: waiting for ArgoCD to apply $1"
  for _ in $(seq 1 72); do
    have="$(kubectl -n "$APP" get deployment magic-server -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null || true)"
    [ "$have" = "$want" ] && break
    sleep 5
  done
  [ "$have" = "$want" ] || die "ArgoCD has not applied $1: kubectl -n argocd get application $APP"
  kubectl -n "$APP" rollout status deployment/magic-server --timeout=300s
  kubectl -n "$APP" rollout status deployment/magic-web --timeout=120s
}


args=()
for arg in "$@"; do
  if [ "$arg" = --even-if-games ]; then EVEN_IF_GAMES=1; else args+=("$arg"); fi
done
set -- ${args[@]+"${args[@]}"}

if [ "${1:-}" = "--rollback" ]; then
  tag="${2:?usage: release.sh --rollback TAG}"
  check_tools
  registry_has magic-server "$tag" && registry_has magic-web "$tag" || die "the registry has no $tag"
  check_games
  point_manifests "$tag"
  rollout "$tag"
  echo "release: the cluster runs $tag"
  exit 0
fi

tag="${1:-$(next_tag)}"
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "a tag looks like v1.2.3, got $tag"
check_tools
check_source "$tag"
ensure_builder
build_and_push "$tag"
check_games
point_manifests "$tag"
git tag -a "$tag" -m "release $tag"
git push -q origin "$tag"
rollout "$tag"
echo "release: the cluster runs $tag"

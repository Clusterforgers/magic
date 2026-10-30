# Deployment

How the game runs on the k3s cluster and how a release reaches it.

## Parts

- `deploy/server.Dockerfile` builds `mtg-server` and `mtg-cli` into one Debian slim image (user 10001); `deploy/web.Dockerfile` builds the client and serves it with nginx (`deploy/nginx.conf`: `/assets` cached for a year, every other path is the client). Both are `linux/amd64`.
- The Helm chart `apps/magic` of `k3s-cluster-manifests` runs them in the namespace `magic`, pinned to `queen`: a Postgres StatefulSet, the server (one replica, `Recreate`, an init container runs `mtg-cli migrate`), the web, one Ingress on `magic.hivemindcloud.dk` (`/api`, `/ws` and `/healthz` to the server, everything else to the web), a post-sync Job that runs `mtg-cli compile`, and a suspended CronJob `magic-ingest` for the card data. The cluster side (secret, DNS, ingest) is in that repo's `docs/magic.md`.
- The server runs behind the nginx ingress: `MTG_TRUSTED_PROXIES=1`, `MTG_COOKIE_SECURE=1`, `MTG_ALLOWED_ORIGINS=https://<host>`. `server.registration` in the chart's `values.yaml` sets `MTG_REGISTRATION`; the picture cache is a volume of `server.imageCacheMiB`. `MTG_SEED` stays unset.
- Games live in memory with their event log in Postgres; a restart resumes them. A release that changes a card's rules or the engine's reading of it fails the stored games that use it at the next boot, so releases go between games.

## Release

`scripts/release.sh [vX.Y.Z]` runs from a clean `main` that equals `origin/main`, on a machine that reaches the registry over Tailscale:

1. builds both images with a persistent buildx builder (`magic-release`, cached between releases) and pushes them to the cluster registry as `magic-server:<tag>` and `magic-web:<tag>`;
2. sets `tag:` in `apps/magic/values.yaml` of the manifests clone (`MTG_MANIFESTS`), commits and pushes;
3. tags the commit and pushes the tag;
4. asks ArgoCD to refresh and waits until both Deployments have rolled out.

Without a tag it takes the next patch after the latest `vX.Y.Z`. `scripts/release.sh --rollback vX.Y.Z` points the cluster at a tag the registry still holds and builds nothing; migrations only move forward, so a rollback across a schema change needs a fix forward instead.

## Card data

The card tables are filled by the CronJob `magic-ingest` (`fetch scryfall --printings`, `fetch decks`, `ingest`, `compile`), started by hand after the first install and whenever Scryfall's data should refresh. Pictures load from the Scryfall CDN as games ask for them and stay in the cache volume.

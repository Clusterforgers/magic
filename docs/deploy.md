# Deployment

How the game runs on the k3s cluster and how a release reaches it.

## Parts

- `deploy/server.Dockerfile` builds `mtg-server` and `mtg-cli` into one Debian slim image (user 10001); `deploy/web.Dockerfile` builds the client and serves it with nginx (`deploy/nginx.conf`: the build's `.gz` twins sent as they are, `/assets` cached for a year, every other path is the client). Both are `linux/amd64`.
- The Helm chart `apps/magic` of `k3s-cluster-manifests` runs them in the namespace `magic`, pinned to `queen`: a Postgres StatefulSet, the server (one replica, `Recreate`, an init container runs `mtg-cli migrate`), the web, one Ingress on `magic.hivemindcloud.dk` (`/api`, `/ws` and `/healthz` to the server, everything else to the web), a post-sync Job that runs `mtg-cli compile --all`, and a suspended CronJob `magic-ingest` for the card data. The cluster side (secret, DNS, ingest) is in that repo's `docs/magic.md`.
- The server runs behind the nginx ingress: `MTG_TRUSTED_PROXIES=1`, `MTG_COOKIE_SECURE=1`, `MTG_ALLOWED_ORIGINS=https://<host>`. `server.registration` in the chart's `values.yaml` sets `MTG_REGISTRATION` and `server.admins` sets `MTG_ADMIN_USERNAMES` (see Accounts); the picture cache is a volume of `server.imageCacheMiB`. `MTG_SEED` stays unset.
- Games live in memory with their event log in Postgres; a restart resumes them. A release that changes a card's rules or the engine's reading of it fails the stored games that use it at the next boot, so releases go between games.

## Release

`scripts/release.sh [vX.Y.Z]` runs from a clean `main` that equals `origin/main`, on a machine that reaches the registry over Tailscale:

1. builds both images with a persistent buildx builder (`magic-release`, cached between releases) and pushes them to the cluster registry as `magic-server:<tag>` and `magic-web:<tag>`;
2. sets `tag:` in `apps/magic/values.yaml` of the manifests clone (`MTG_MANIFESTS`), commits and pushes;
3. tags the commit and pushes the tag;
4. asks ArgoCD to refresh and waits until both Deployments have rolled out.

Between steps 1 and 2 it refuses while a game is running, since the restart cuts the sockets of its seats (`--even-if-games` overrides); the pushed images stay in the registry, so once the game ends `--rollback <tag>` rolls them out and the commit is tagged by hand.

Without a tag it takes the next patch after the latest `vX.Y.Z`. `scripts/release.sh --rollback vX.Y.Z` points the cluster at a tag the registry still holds and builds nothing; migrations only move forward, so a rollback across a schema change needs a fix forward instead.

## Accounts

`MTG_ADMIN_USERNAMES` (comma-separated; chart `server.admins`, a list) names the accounts that make the others (`POST /api/admin/users`, the Accounts page of the client); with `MTG_REGISTRATION=closed` nobody else can register. `MTG_ADMIN_USERNAME` (chart `server.admin`) adds one more name. The server marks every named account when it starts and clears the flag of every other account, so an account taken off the list loses admin at the next start. A name that belongs to no account promotes nobody and logs a warning at start (the setting does not create an account).

A fresh install therefore goes in this order:

1. Install with `registration: open` and no admins, and register the owner's account through the client.
2. Set `server.admins` to that username and `server.registration` to `closed`, and roll the server out.
3. Sign in as the owner: the account menu shows Accounts, where friends are added, given a new password when they forget theirs and removed; they change the password under Account settings.

Until step 2 anyone who reaches the site can register, and could take the owner's name first, so step 1 follows the install at once. Another admin is an existing account added to `server.admins`, then a rollout.

No admin account can be deleted, in Account settings or from the Accounts page, and no admin sets another admin's password, so the owners cannot lock the house by accident. An admin's own password changes under Account settings; for a forgotten one, take the name off `server.admins`, roll out, set a new password from another admin's Accounts page and add the name back.

### A lost or mistyped admin

If `server.admins` names no account (a typo) or the owner needs a fresh account, nobody can make accounts. Recover with the same steps as an install:

1. Set `server.registration` to `open`, add the name the new account will have to `server.admins`, and roll the server out.
2. Register that account through the client.
3. Set `server.registration` back to `closed`, roll the server out, and sign in: the account menu shows Accounts.

The rollout in step 3 makes the new account an admin; a name taken off the list is a plain account from the next rollout on. Accounts and decks of the friends stay as they are. Do the steps without a pause, since anyone could register the name while registration is open.

## Backups

A CronJob dumps the database every night onto its own volume next to the database on queen and keeps the newest 14 (restore and copy-out: the manifests repo's `docs/magic.md`). Copy a dump off the machine from time to time (roadmap item d1c).

## Card data

The card tables are filled by the CronJob `magic-ingest` (`fetch scryfall --printings`, `fetch decks`, `ingest`, `compile --all`), started by hand after the first install and whenever Scryfall's data should refresh. Pictures load from the Scryfall CDN as games ask for them and stay in the cache volume.

## Operations

Read-only checks of the running game, from a machine that reaches the cluster:

- State: `kubectl -n magic get deploy,pods` and `curl -s https://magic.hivemindcloud.dk/healthz`; the server log is `kubectl -n magic logs deploy/magic-server -c server`.
- Cards players marked as important to fix: `kubectl -n magic exec deploy/magic-server -c server -- mtg-cli fix-marks --limit 30`.
- Games that ended in `failed` (each is an engine or server bug to reproduce): `kubectl -n magic exec magic-db-0 -- psql -U magic -d magic -Atc "select id, failure_reason, failure from games where status = 'failed' order by created_at desc limit 10"`; `select status, count(*) from games group by 1` counts them all.

# Deployment

How the game runs on the k3s cluster and how a release reaches it.

## Parts

- `deploy/server.Dockerfile` builds `mtg-server` and `mtg-cli` into one Debian slim image (user 10001); `deploy/web.Dockerfile` builds the client and serves it with nginx (`deploy/nginx.conf`: the build's `.gz` twins sent as they are, `/assets` cached for a year, every other path is the client). Both are `linux/amd64`.
- The Helm chart `apps/magic` of `k3s-cluster-manifests` runs them in the namespace `magic`, pinned to `queen`: a Postgres StatefulSet, the server (one replica, `Recreate`, an init container runs `mtg-cli migrate`), the web, one Ingress on `magic.hivemindcloud.dk` (`/api`, `/ws` and `/healthz` to the server, everything else to the web), a post-sync Job that runs `mtg-cli compile --all`, and the daily CronJob `magic-ingest` for the card data (Card data, below). The cluster side (secret, DNS, ingest) is in that repo's `docs/magic.md`.
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

A CronJob dumps the database every night onto its own volume next to the database on queen and keeps the newest 14 (restore and copy-out: the manifests repo's `docs/magic.md`). Copy a dump off the machine from time to time.

## Card data

The card tables are filled by the CronJob `magic-ingest`, which runs `mtg-cli refresh` every day at 04:00 Copenhagen time: it fetches Scryfall and MTGJSON, loads and compiles what changed, takes a coverage snapshot and records the run for the Card support page ([data-sources.md](data-sources.md), `refresh`). The cache is a volume of its own, so a day with no upstream change downloads only Scryfall's listing and MTGJSON's index and loads nothing. While a game runs the job loads nothing and records the run as deferred, since a server restart after a rules change fails a stored game (as for releases); the next run loads it. The post-sync compile Job also stores a snapshot, so each release shows in the feed with the cards it made automatic. Pictures load from the Scryfall CDN as games ask for them and stay in the picture volume.

The chart's `templates/jobs.yaml` for this:

```yaml
# magic-compile: the container's command compiles, then stores a snapshot for the Card support page.
            - |
              out=$(mtg-cli compile --all 2>&1) && { echo "$out"; mtg-cli coverage --all --snapshot || true; exit 0; }
              echo "$out"
              case "$out" in *"no cards are stored"*) exit 0 ;; esac
              exit 1
# magic-compile: one more env entry.
            - name: MTG_RELEASE
              value: {{ .Values.tag | quote }}
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: magic-card-cache
spec:
  accessModes: [ReadWriteOnce]
  storageClassName: {{ .Values.persistence.storageClass }}
  resources:
    requests:
      storage: 4Gi
---
apiVersion: batch/v1
kind: CronJob
metadata:
  name: magic-ingest
spec:
  schedule: "0 4 * * *"
  timeZone: Europe/Copenhagen
  concurrencyPolicy: Forbid
  startingDeadlineSeconds: 3600
  successfulJobsHistoryLimit: 3
  failedJobsHistoryLimit: 3
  jobTemplate:
    spec:
      backoffLimit: 0
      template:
        spec:
          restartPolicy: Never
          nodeSelector:
            {{- toYaml .Values.nodeSelector | nindent 12 }}
          {{- include "magic.podSecurity" . | nindent 10 }}
          containers:
            - name: ingest
              image: {{ include "magic.serverImage" . }}
              command: [mtg-cli, refresh]
              env:
                {{- include "magic.databaseEnv" . | nindent 16 }}
                - name: MTG_CACHE_DIR
                  value: /cache
                - name: MTG_RELEASE
                  value: {{ .Values.tag | quote }}
              resources:
                requests: {cpu: 200m, memory: 512Mi}
                limits: {memory: 4Gi}
              volumeMounts:
                - name: cache
                  mountPath: /cache
          volumes:
            - name: cache
              persistentVolumeClaim:
                claimName: magic-card-cache
```

`suspend: true` goes; a refresh by hand stays `kubectl -n magic create job --from=cronjob/magic-ingest ingest-$(date +%F)`, and a one-off Job running `mtg-cli refresh --force` loads even when nothing changed. The first run on the new volume downloads every file once (about 600 MB); since the inputs equal the stored ones, ingest skips them.

## Operations

Read-only checks of the running game, from a machine that reaches the cluster:

- State: `kubectl -n magic get deploy,pods` and `curl -s https://magic.hivemindcloud.dk/healthz`; the server log is `kubectl -n magic logs deploy/magic-server -c server`.
- Cards players marked as important to fix: `kubectl -n magic exec deploy/magic-server -c server -- mtg-cli fix-marks --limit 30`.
- Games that ended in `failed` (each is an engine or server bug to reproduce): `kubectl -n magic exec magic-db-0 -- psql -U magic -d magic -Atc "select id, failure_reason, failure from games where status = 'failed' order by created_at desc limit 10"`; `select status, count(*) from games group by 1` counts them all.

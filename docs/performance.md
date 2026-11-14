# Performance

How fast the client loads, how it is measured, and what the numbers are.

## Method

- Live site as a guest from the LAN (RTT about 4 ms) and a local production build behind `deploy/nginx.conf` (nginx in Docker), both in headless Chromium through the DevTools protocol: cold cache, then warm; throttled runs use 60 ms latency and 10 Mbit/s. Each API call the pages make is timed with `curl`.
- Bundles: `vite build` with source maps, sizes per chunk at gzip level 9; `bundleBudget.test.ts` holds the budgets.

## Measured causes

| Area | Measured | Cause |
| --- | --- | --- |
| Card pictures | a 4-seat game's warm-up: 369 pictures in 82 s; a miss 0.1 to 1.4 s, a hit 17 ms | one CDN request at a time plus 100 ms, so a picture a player opens queues behind the warm-up |
| Release | every chunk renamed, a returning browser fetches 241 kB gzip again | React and the router in the entry chunk with the app code |
| Release | card search 0.7 to 1.6 s, `COMMIT` 1.1 to 1.5 s, precon index built 72 s at boot | the compile job and the boot work share queen's disk and CPU; the same queries take 2 to 17 ms locally |
| Compression | entry 150 kB on the wire, 128 kB at level 9 | nginx gzip level 1 on the fly |
| Font | the Latin font starts 730 ms into a throttled cold load | found only after the first render |
| Waterfall | page, entry and its preloads, the route chunk and its small chunks, then the page's API calls | route chunks load after the entry runs |

The rest is fast: page TTFB 4 to 15 ms, `auth/me`, `auth/options` and `lobbies` 2 to 10 ms, `precons` 200 to 250 ms (185 kB, 23 kB gzip), hashed assets immutable for a year, the page `no-cache`, pictures a week with an `ETag`, JSON gzipped.

## Before and after

| Measure | Before | After |
| --- | --- | --- |
| Pictures fetched at once from the CDN | 1, 100 ms apart | 8, starts 25 ms apart (warm-up 4) |
| 40 pictures at 200 ms each (unit test) | 12 s | under 1.4 s |
| Lobby, cold, throttled: requests, bytes | 52, 351 kB | 49, 310 kB |
| Lobby, cold, throttled: DOM ready, first paint | 391 ms, 758 ms | 375 ms, 748 ms |
| Font request start (throttled) | 729 ms | 66 ms |
| First page after a release that changes app code only (gzip) | 241 kB | 137 kB |
| Entry chunk on the wire | 150 kB (gzip 1) | 60 kB entry + 94 kB vendor at gzip 9 |

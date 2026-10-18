# Frontend spike: React stack

Results of the time-boxed spike that chose the web stack: React + Vite + Tailwind/shadcn + TanStack Router + zustand + dnd-kit + Motion. The spike code is in git history at `446bdd0^`.

## Setup

4 players, 176 objects (148 permanents + 28 hand cards), 6 state patches per 100 ms tick, a hand-to-battlefield move every 2 s. Production build, headless Chromium 155 with software rendering, run via `pnpm e2e`.

## Results

| Criterion | Target | Result |
|---|---|---|
| Frame rate under patches | >= 55 fps | 60 fps, p95 frame 16.7 ms |
| Main-thread busy fraction | <= 25% | 11% (4x CPU throttle still held 60 fps) |
| Hover preview latency p95 | < 50 ms | ~1 ms |
| Zone-move animation | intermediate positions | 28 distinct positions |
| Click and drag targeting | legal highlight, resolve | pass |

## Findings

- Per-card store subscriptions plus unchanged-object identity keep re-renders to the patched cards.
- dnd-kit `disabled` marks elements `aria-disabled`, which makes legal targets look disabled to assistive tech. Drag attributes are applied only to playable cards.
- Headless rAF is vsync-capped, so frame rate alone hides headroom; the busy-fraction test is the real signal.
- Limit: the run is software-rendered headless, not a real GPU.

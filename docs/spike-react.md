# M0 frontend spike: React stack

Verdict: **React + Vite + Tailwind/shadcn + TanStack Router + zustand + dnd-kit + Motion is locked in.**

The spike code was removed in M2g (last present in `446bdd0^`); this page keeps its results and findings. Test names below refer to that code.

Setup: 4 players, 176 objects (148 permanents + 28 hand cards), 6 state patches per 100 ms tick, hand-to-battlefield move every 2 s. Production build, headless Chromium 155 (software rendering), run via `pnpm e2e`.

| Criterion | Target | Result |
|---|---|---|
| Frame rate under patches | >= 55 fps | 60 fps, p95 frame 16.7 ms |
| Main-thread busy fraction | <= 25% | 11% (4x CPU throttle still held 60 fps) |
| Hover preview latency p95 | < 50 ms | ~1 ms |
| Zone-move animation | intermediate positions | 28 distinct positions |
| Click and drag targeting | legal highlight, resolve | pass |

Findings:
- Per-card store subscriptions plus unchanged-object identity keep re-renders to the patched cards (unit-tested in `game/store.test.ts`).
- dnd-kit `disabled` marks elements `aria-disabled`, which wrongly makes legal targets look disabled to assistive tech. Drag attributes are only applied to playable cards.
- Headless rAF is vsync-capped, so frame rate alone hides headroom; the busy-fraction test is the real signal.
- Limits: software-rendered headless run, not a real GPU; revisit with a real-browser profile at M4 with real card art.

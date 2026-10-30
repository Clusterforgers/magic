/**
 * The engine seed each e2e server starts with (`MTG_SEED`): every game on the server shuffles the same way, so a
 * failing game replays exactly. `E2E_SEED=<n>` replays another seed on both servers and `E2E_SEED=random` leaves
 * the engine random again; the server logs the seed of every game it creates (`game created`).
 */
const pick = (pinned: number): string => {
  const set = process.env.E2E_SEED
  if (set === undefined || set === '') return String(pinned)
  return set === 'random' ? '' : set
}

/** The in-memory server (`full-game.spec.ts`). */
export const GAME_SEED = pick(1)

/** The server on the scratch database, shared by `precon-game.spec.ts` and `commander-four.spec.ts`. */
export const ACCOUNTS_SEED = pick(1)

import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'

/**
 * PLACEHOLDER for the N-seat table (next task: battlefields, hand, stack, log and the decision prompts).
 *
 * `GamePage` renders it, with no props, while the game is being played. Read live state with `usePlay(selector)` and
 * act through `useGame().session`; both come from `features/game/gameContext`. Replace this body, keep the name.
 */
export function GameTable() {
  const { seatCount, phase, stage } = usePlay(
    useShallow((state) => ({
      seatCount: state.seatCount,
      phase: state.phase,
      stage: state.view?.stage ?? null,
    })),
  )
  return (
    <section
      aria-label="Game table"
      data-testid="game-table-placeholder"
      className="mx-auto grid w-full max-w-2xl gap-2 px-4 py-6 sm:px-6"
    >
      <h1 className="text-2xl font-semibold tracking-tight">The game is on</h1>
      <p className="text-sm text-muted-foreground">The table is not built yet. This is its placeholder.</p>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-muted-foreground">Seats</dt>
        <dd>{seatCount ?? 'unknown'}</dd>
        <dt className="text-muted-foreground">Phase</dt>
        <dd>{phase}</dd>
        {stage === null ? null : (
          <>
            <dt className="text-muted-foreground">Stage</dt>
            <dd>{stage}</dd>
          </>
        )}
      </dl>
    </section>
  )
}

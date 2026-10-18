import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { fixtureGame } from './devGame'
import { FIXTURE_MARKER } from './fixture'

/**
 * `/dev/table?seats=2|4|12&mode=grid|focus&focus=<seat>&prompt=priority`: the real table over a fixture game, for
 * looking at it. `prompt` names a decision from `dev/prompts.ts` for the viewer to face; answering it clears it.
 */
export function DevTableRoute() {
  const { seats, mode, focus, prompt } = useSearch({ from: '/dev/table' })
  const game = useMemo(() => fixtureGame({ seats, prompt }), [seats, prompt])
  const options = useMemo(() => ({ mode, focus }), [mode, focus])
  return (
    <GameContext value={game}>
      <TableOptionsContext value={options}>
        <p className="sr-only" data-fixture={FIXTURE_MARKER}>
          Development fixture with {seats} seats
        </p>
        <GameTable />
      </TableOptionsContext>
    </GameContext>
  )
}

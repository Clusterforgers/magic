import { useSearch } from '@tanstack/react-router'
import { useMemo } from 'react'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { DevFxControls } from './DevFxControls'
import { fixtureGame } from './devGame'
import { FIXTURE_MARKER } from './fixture'
import { parseStopSpec } from './stopSpec'

/**
 * `/dev/table?seats=2|4|12&mode=grid|focus&focus=<seat>&prompt=priority&turn=you&fx=damage&slow=5&preview=3000`: the
 * real table over a fixture game, for looking at it. `prompt` names a decision from `dev/prompts.ts` for the viewer to
 * face; answering it clears it. `turn=you` makes it the viewer's own turn and `step=main2` (any step's name) puts the
 * turn in that step, to look at the theme of its phase. `fx` plays one of the moves of
 * `dev/fxScenarios.ts` once the page has loaded (buttons in the page play them again) and `slow` stretches the
 * table effects, so one can be looked at before it is gone. `preview` is how many milliseconds the table takes to
 * answer a preview of blocks or attacks (120 when missing), so "Checking outcome…" can be looked at. `stops` lights stops
 * (`own:main1,opp:end:keep`: side, step, and `keep` for a lasting one) and `fc=next|held` sets Full Control; the
 * ladder and the toggle then answer as a server would.
 */
export function DevTableRoute() {
  const { seats, mode, focus, prompt, turn, step, hand, fx, slow, preview, stops, fc } = useSearch({
    from: '/dev/table',
  })
  const game = useMemo(
    () =>
      fixtureGame({
        seats,
        prompt,
        turn,
        step,
        hand,
        previewMs: preview,
        stops: stops === undefined ? undefined : parseStopSpec(stops),
        fullControl: fc === 'next' ? 'NextWindow' : fc === 'held' ? 'Held' : undefined,
      }),
    [seats, prompt, turn, step, hand, preview, stops, fc],
  )
  // Asking for an effect turns the layer on, even where it is off by default.
  const options = useMemo(
    () => ({ mode, focus, effects: fx === undefined ? undefined : true, effectsSlow: slow }),
    [mode, focus, fx, slow],
  )
  return (
    <GameContext value={game}>
      <TableOptionsContext value={options}>
        <p className="sr-only" data-fixture={FIXTURE_MARKER}>
          Development fixture with {seats} seats
        </p>
        <GameTable />
        <DevFxControls initial={fx} />
      </TableOptionsContext>
    </GameContext>
  )
}

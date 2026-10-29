import { useSearch } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'
import { cardsApi } from '@/net/cards'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { DevFxControls } from './DevFxControls'
import { fixtureGame } from './devGame'
import { FIXTURE_MARKER, buildFixture } from './fixture'
import { SHOWCASE_NAMES, fetchRealIds } from './realCards'
import type { RealIds } from './realCards'
import { parseStopSpec } from './stopSpec'

/** The oracle ids of the fixture's cards from the server (`?cards=real`), read once; none until they arrive. */
function useRealIds(on: boolean, seats: number, hand: number | undefined): RealIds | undefined {
  const [ids, setIds] = useState<RealIds | undefined>(undefined)
  useEffect(() => {
    if (!on) return
    let live = true
    const names = buildFixture({ seats, hand }).view.objects.map((object) => object.name)
    void fetchRealIds(cardsApi, [...names, ...SHOWCASE_NAMES]).then((found) => {
      if (live) setIds(found)
    })
    return () => {
      live = false
    }
  }, [on, seats, hand])
  return on ? ids : undefined
}

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
  const { seats, mode, focus, prompt, turn, step, hand, fx, slow, preview, stops, fc, rules, cards } =
    useSearch({
      from: '/dev/table',
    })
  const real = useRealIds(cards === 'real', seats, hand)
  const game = useMemo(
    () =>
      fixtureGame({
        seats,
        prompt,
        turn,
        step,
        hand,
        rules,
        previewMs: preview,
        stops: stops === undefined ? undefined : parseStopSpec(stops),
        fullControl: fc === 'next' ? 'NextWindow' : fc === 'held' ? 'Held' : undefined,
        real,
      }),
    [seats, prompt, turn, step, hand, rules, preview, stops, fc, real],
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

import { useSearch } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'
import { cardsApi } from '@/net/cards'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { setDetailFetcher } from '@/features/table/printed'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { DevFxControls } from './DevFxControls'
import { DevTurnChange } from './DevTurnChange'
import { fixtureGame } from './devGame'
import { FIXTURE_MARKER, buildFixture } from './fixture'
import { FRONT_OF, SHOWCASE_NAMES, fetchRealIds } from './realCards'
import type { RealIds } from './realCards'
import { printedDetail, STATES_BASE_PERMANENTS, withStates } from './states'
import { parseStopSpec } from './stopSpec'
import { menuCandidates, useOpenDevMenu } from '../manual/devMenu'
import { useOpenDevPanel } from '../manual/panel/devPanel'

/** The oracle ids of the fixture's cards from the server (`?cards=real`), read once; none until they arrive. */
function useRealIds(
  on: boolean,
  seats: number,
  hand: number | undefined,
  states: boolean,
): RealIds | undefined {
  const [ids, setIds] = useState<RealIds | undefined>(undefined)
  useEffect(() => {
    if (!on) return
    let live = true
    const table = states
      ? withStates(buildFixture({ seats, hand, permanents: STATES_BASE_PERMANENTS }))
      : buildFixture({ seats, hand })
    const names = table.view.objects.map((object) => object.name)
    void fetchRealIds(cardsApi, [...names, ...SHOWCASE_NAMES, ...Object.values(FRONT_OF)]).then((found) => {
      if (live) setIds(found)
    })
    return () => {
      live = false
    }
  }, [on, seats, hand, states])
  return on ? ids : undefined
}

/**
 * `/dev/table?seats=2|4|12&mode=grid|focus&focus=<seat>&prompt=priority&turn=you&fx=damage&bar=1&slow=5&preview=3000`: the
 * real table over a fixture game, for looking at it. `prompt` names a decision from `dev/prompts.ts` for the viewer to
 * face; answering it clears it. `turn=you` makes it the viewer's own turn and `step=main2` (any step's name) puts the
 * turn in that step, to look at the theme of its phase. `fx` plays one of the moves of `dev/fxScenarios.ts` once the
 * page has loaded (the Effects bar plays them again; it is folded behind one button, and `bar=1` opens it) and `slow`
 * stretches the table effects, so one can be looked at before it is gone; `newturn=1` passes the turn to the other side
 * once, for the turn banner and badge. `preview` is how many milliseconds the table takes to
 * answer a preview of blocks or attacks (120 when missing), so "Checking outcome…" can be looked at. `stops` lights stops
 * (`own:main1,opp:end:keep`: side, step, and `keep` for a lasting one) and `fc=next|held` sets Full Control; the
 * ladder and the toggle then answer as a server would.
 */
export function DevTableRoute() {
  const {
    seats,
    board,
    mode,
    focus,
    prompt,
    turn,
    step,
    hand,
    fx,
    newturn,
    bar,
    slow,
    preview,
    stops,
    fc,
    away,
    rules,
    cards,
    states,
    equip,
    reveal,
    menu,
    panel,
  } = useSearch({
    from: '/dev/table',
  })
  const real = useRealIds(cards === 'real', seats, hand, states === true)
  const game = useMemo(() => {
    // The state cards compare with printed numbers that come from the fixture, unless real cards bring their own.
    if (states === true) setDetailFetcher(cards === 'real' ? (id) => cardsApi.get(id) : printedDetail)
    return fixtureGame({
      states,
      equip,
      reveal,
      seats,
      board,
      prompt: panel === 'top' ? 'look-top' : prompt,
      holding: menu === undefined && panel === undefined ? undefined : true,
      turn,
      step,
      hand,
      rules,
      previewMs: preview,
      stops: stops === undefined ? undefined : parseStopSpec(stops),
      fullControl: fc === 'next' ? 'NextWindow' : fc === 'held' ? 'Held' : undefined,
      away,
      real,
    })
  }, [
    seats,
    board,
    prompt,
    menu,
    panel,
    turn,
    step,
    hand,
    rules,
    states,
    equip,
    reveal,
    cards,
    preview,
    stops,
    fc,
    away,
    real,
  ])
  const view = game.session.store.getState().view
  const candidates = useMemo(
    () => (menu === undefined || view === null ? undefined : menuCandidates(view, view.viewer, menu)),
    [menu, view],
  )
  useOpenDevMenu(candidates)
  const panelCards = useMemo(
    () => (panel !== 'card' || view === null ? undefined : menuCandidates(view, view.viewer, 'permanent')),
    [panel, view],
  )
  useOpenDevPanel(panel === 'top' ? 'look' : panel, panelCards)
  // Asking for an effect turns the layer on, even where it is off by default.
  const options = useMemo(
    () => ({
      mode,
      focus,
      effects: fx === undefined && newturn !== true ? undefined : true,
      effectsSlow: slow,
    }),
    [mode, focus, fx, newturn, slow],
  )
  return (
    <GameContext value={game}>
      <TableOptionsContext value={options}>
        <p className="sr-only" data-fixture={FIXTURE_MARKER}>
          Development fixture with {seats} seats
        </p>
        <GameTable />
        <DevFxControls initial={fx} open={bar} />
        <DevTurnChange on={newturn === true} />
      </TableOptionsContext>
    </GameContext>
  )
}

/** The dev table's `?panel=top`: the viewer has just looked at the top three cards of its library, which the Commands panel lists to arrange. */
import { LOOKED_MOMENTS } from './lookedPrompts'
import type { Fixture } from './fixture'

/** How many cards the scene looks at. */
const LOOKED = 3

/** The fixture with three cards of the viewer's library looked at by hand: `known_top` and the log lines of the look. */
export function lookTopMoment(fixture: Fixture): Fixture {
  const withLibrary = LOOKED_MOMENTS.scry?.(fixture) ?? fixture
  const { view } = withLibrary
  const cards = view.objects
    .filter((o) => typeof o.zone === 'object' && 'Library' in o.zone && o.zone.Library === view.viewer)
    .map((o) => o.id)
    .slice(0, LOOKED)
  const seq = (withLibrary.log.at(-1)?.seq ?? -1) + 1
  return {
    ...withLibrary,
    view: {
      ...view,
      players: view.players.map((player) =>
        player.seat === view.viewer ? { ...player, known_top: cards } : player,
      ),
    },
    log: [
      ...withLibrary.log,
      {
        seq,
        event: { Public: { ManualAction: { player: view.viewer, action: { LookTop: { count: LOOKED } } } } },
      },
      { seq: seq + 1, event: { Public: { LibraryLooked: { player: view.viewer, cards } } } },
    ],
  }
}

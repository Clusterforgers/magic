/** The dev table's `?moments=1`: one batch holding a reveal of three cards and a d20 roll, to look at both moments over the table. */
import type { PlayState } from '@/play/store'
import type { EventBatch, ObjectView } from '@/protocol/generated'
import { tableOf } from './fxScenarios'

/** How many cards the reveal shows. */
export const REVEALED_CARDS = 3

/** The result the die lands on. */
export const ROLL_RESULT = 17

const inHand = (object: ObjectView, seat: number): boolean =>
  typeof object.zone === 'object' && 'Hand' in object.zone && object.zone.Hand === seat

/**
 * The batch of the first opponent revealing cards of its hand (copies of the viewer's hand cards under new ids) and the
 * viewer rolling a d20. Null without a table, an opponent or a hand.
 */
export function momentsBatch(state: PlayState): EventBatch | null {
  const { view, seq } = state
  if (view === null || seq === null) return null
  const rival = view.players.find((player) => player.seat !== view.viewer)?.seat
  const hand = view.objects.filter((object) => inHand(object, view.viewer)).slice(0, REVEALED_CARDS)
  if (rival === undefined || hand.length === 0) return null
  const base = Math.max(...view.objects.map((object) => object.id)) + 1
  const shown = hand.map((card, index): ObjectView => ({
    ...card,
    id: base + index,
    owner: rival,
    controller: rival,
    zone: { Hand: rival },
  }))
  const next = structuredClone(view)
  // A batch that changes nothing in the view is not told apart from a decision, so the turn counter moves.
  next.turn = { ...next.turn, number: next.turn.number + 1 }
  return {
    from_seq: seq,
    events: [
      { Public: { Revealed: { player: rival, cards: shown.map((card) => card.id) } } },
      { Public: { DieRolled: { player: view.viewer, sides: 20, result: ROLL_RESULT } } },
    ],
    table: tableOf(next),
    objects: [],
    gone: [],
    shown,
  }
}

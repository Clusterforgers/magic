/**
 * The dev table's `?reveal=1`: what Reveal and Note put in the view. The first opponent shows two cards of its hand and
 * the top card of its library, the viewer shows one card of its hand, and one permanent of each carries a note.
 */
import type { ObjectView, PlayerView } from '@/protocol/generated'
import { rowOf } from '../cardModel'
import type { Fixture } from './fixture'

/** The note on the viewer's permanent. */
export const OWN_NOTE = 'Back to hand next turn'

/** The note on the opponent's permanent: more than the mark's first words, to look at the cut. */
export const RIVAL_NOTE = 'Holds the trick until my end step'

const inHand = (object: ObjectView, seat: number): boolean =>
  typeof object.zone === 'object' && 'Hand' in object.zone && object.zone.Hand === seat

/** The fixture with the first opponent's hand cards and top card revealed, the viewer's first hand card revealed, and two notes. */
export function withRevealed(fixture: Fixture): Fixture {
  const { view } = fixture
  const rival = view.players.find((player) => player.seat !== view.viewer)?.seat
  const hand = view.objects.filter((object) => inHand(object, view.viewer))
  const [own, first, second, third] = hand
  if (rival === undefined || own === undefined || first === undefined || second === undefined) return fixture
  const top = third ?? second
  const base = Math.max(...view.objects.map((object) => object.id)) + 1
  const shown: ObjectView[] = [
    { ...first, id: base, owner: rival, controller: rival, zone: { Hand: rival } },
    { ...second, id: base + 1, owner: rival, controller: rival, zone: { Hand: rival } },
    { ...top, id: base + 2, owner: rival, controller: rival, zone: { Library: rival } },
  ]
  const board = view.objects.filter(
    (object) => object.zone === 'Battlefield' && rowOf(object) === 'creatures',
  )
  const mine = board.find((object) => object.controller === view.viewer)
  const theirs = board.find((object) => object.controller === rival)
  const noted = (object: ObjectView): ObjectView => {
    if (object.id === mine?.id) return { ...object, note: OWN_NOTE }
    return object.id === theirs?.id ? { ...object, note: RIVAL_NOTE } : object
  }
  const players = view.players.map((player): PlayerView => {
    if (player.seat === view.viewer) return { ...player, revealed_hand: [own.id] }
    if (player.seat !== rival) return player
    return {
      ...player,
      hand: { ...player.hand, count: Math.max(player.hand.count, 2) },
      revealed_hand: [base, base + 1],
      revealed_top: base + 2,
    }
  })
  return { ...fixture, view: { ...view, players, objects: [...view.objects.map(noted), ...shown] } }
}

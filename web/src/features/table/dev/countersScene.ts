/**
 * The dev table's `?counters=1`: counters and effects on every player. The viewer has poison and energy, the first
 * opponent all five kinds with an icon in counts of two digits, and every other seat rad and one more kind, so each pod
 * density shows them; the viewer can't lose and has hexproof, the first opponent can't gain life and plays with an
 * extra land, and every other seat has no maximum hand size.
 */
import type { CounterKind, PlayerView } from '@/protocol/generated'
import type { Fixture } from './fixture'

const rad: CounterKind = { Other: 'rad' }
const energy: CounterKind = { Other: 'energy' }
const experience: CounterKind = { Other: 'experience' }
const ticket: CounterKind = { Other: 'ticket' }

/** The counters `seat` gets, by its place after the viewer. */
function countersFor(place: number): PlayerView['counters'] {
  if (place === 0)
    return [
      ['poison', 3],
      [energy, 4],
    ]
  if (place === 1)
    return [
      ['poison', 9],
      [rad, 12],
      [energy, 15],
      [experience, 2],
      [ticket, 1],
    ]
  return place % 2 === 0
    ? [[rad, place + 1]]
    : [
        [rad, 3],
        [experience, place],
      ]
}

/** The effects `seat` gets, by its place after the viewer. */
function effectsFor(place: number): PlayerView['effects'] {
  if (place === 0) return ['CantLose', { Keyword: 'Hexproof' }]
  if (place === 1) return ['CantGainLife', { ExtraLandPlays: 1 }, 'HandRevealed']
  return ['NoMaxHandSize']
}

/** The fixture with counters and effects on every player. */
export function withPlayerCounters(fixture: Fixture): Fixture {
  const { view } = fixture
  const seats = view.players.length
  const players = view.players.map((player) => ({
    ...player,
    counters: countersFor((player.seat - view.viewer + seats) % seats),
    effects: effectsFor((player.seat - view.viewer + seats) % seats),
  }))
  return { ...fixture, view: { ...view, players } }
}

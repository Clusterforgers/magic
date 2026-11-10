/** The dev table's `?prompt=priority-top`: the viewer may look at the top card of its library any time and cast it (Sigarda, Future Sight). */
import type { ObjectView } from '@/protocol/generated'
import type { PendingDecision } from '@/play/store'
import type { Fixture } from './fixture'

/** The name the top card takes, a creature Sigarda lets its controller cast. */
export const TOP_CARD_NAME = 'Serra Angel'

/** The viewer's top library card, as `withTopCard` adds it. */
const topOf = (fixture: Fixture): ObjectView | undefined =>
  fixture.view.objects.find(
    (object) =>
      typeof object.zone === 'object' &&
      'Library' in object.zone &&
      object.zone.Library === fixture.view.viewer,
  )

/** The fixture with a creature on top of the viewer's library that the viewer sees (`known_top`). */
export function withTopCard(fixture: Fixture): Fixture {
  const { view } = fixture
  if (topOf(fixture) !== undefined) return fixture
  const spell = view.objects.find(
    (object) =>
      typeof object.zone === 'object' &&
      'Hand' in object.zone &&
      object.zone.Hand === view.viewer &&
      object.types.includes('Creature'),
  )
  if (spell === undefined) return fixture
  const id = Math.max(...view.objects.map((object) => object.id)) + 1
  const top: ObjectView = {
    ...structuredClone(spell),
    id,
    name: TOP_CARD_NAME,
    zone: { Library: view.viewer },
    subtypes: ['Angel'],
  }
  return {
    ...fixture,
    view: {
      ...view,
      objects: [...view.objects, top],
      players: view.players.map((player) =>
        player.seat === view.viewer ? { ...player, known_top: [id] } : player,
      ),
    },
  }
}

/** `base` with the viewer's seen top card among the castable cards. */
export function withTopCast(
  base: (fixture: Fixture) => PendingDecision,
): (fixture: Fixture) => PendingDecision {
  return (fixture) => {
    const pending = base(fixture)
    const { kind } = pending.decision
    const top = topOf(fixture)
    if (!('Priority' in kind) || top === undefined) return pending
    const castable = [...kind.Priority.castable, top.id]
    return {
      ...pending,
      decision: { ...pending.decision, kind: { Priority: { ...kind.Priority, castable } } },
    }
  }
}

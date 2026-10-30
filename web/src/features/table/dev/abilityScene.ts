/** Priority with abilities to activate on the table, for `?prompt=priority-abilities`: the dev table's look at legal permanents, graveyard and command cards. */
import type { PendingDecision } from '@/play/store'
import type { AbilityRef, ActivatedView, ObjectId, ObjectView } from '@/protocol/generated'
import type { Fixture } from './fixture'
import { creaturesOf } from './effectPrompts'

const ref = (object: ObjectId, ability: number): AbilityRef => ({ object, ability })

/** The abilities the scene gives: a creature with one, a creature with two, a graveyard card and the commander. */
function wordsOf(fixture: Fixture): Map<ObjectId, ActivatedView[]> {
  const { view } = fixture
  const me = view.players[view.viewer]
  const [one, two] = creaturesOf(view, view.viewer).filter((creature) => creature.permanent?.tapped === false)
  const yard = me?.graveyard.cards?.findLast(
    (id) => view.objects.find((object) => object.id === id)?.types.includes('Land') === false,
  )
  const commander = me?.command.cards?.at(0)
  const words = new Map<ObjectId, ActivatedView[]>()
  const give = (id: ObjectId | undefined, ...entries: Omit<ActivatedView, 'ability'>[]): void => {
    if (id === undefined) return
    words.set(
      id,
      entries.map((entry, index) => ({ ...entry, ability: ref(id, index) })),
    )
  }
  give(one?.id, { cost: '{2}, {T}', effect: 'Draw a card.' })
  give(
    two?.id,
    { cost: '{T}', effect: 'Scry 1.' },
    { cost: '{1}', effect: 'Attach to target creature you control.', keyword: 'Equip {1}' },
  )
  give(yard, {
    cost: '{3}{B}, Exile this card from your graveyard',
    effect: 'Create a 2/2 black Zombie creature token.',
  })
  give(commander, { cost: '{T}', effect: 'Scry 1.' })
  return words
}

/** The fixture with those abilities worded on the viewer's cards. */
export function abilitiesTable(fixture: Fixture): Fixture {
  const words = wordsOf(fixture)
  const objects = fixture.view.objects.map((object): ObjectView =>
    words.has(object.id) ? { ...object, activated: words.get(object.id) } : object,
  )
  return { ...fixture, view: { ...fixture.view, objects } }
}

/** `base` with every ability worded on a card of the table (`abilitiesTable`) offered as one to activate. */
export function withAbilities(
  base: (fixture: Fixture) => PendingDecision,
): (fixture: Fixture) => PendingDecision {
  return (fixture) => {
    const pending = base(fixture)
    const { kind } = pending.decision
    if (!('Priority' in kind)) return pending
    const abilities = fixture.view.objects.flatMap((object) =>
      (object.activated ?? []).flatMap((entry) => (entry.ability === undefined ? [] : [entry.ability])),
    )
    return {
      ...pending,
      decision: { ...pending.decision, kind: { Priority: { ...kind.Priority, abilities } } },
    }
  }
}

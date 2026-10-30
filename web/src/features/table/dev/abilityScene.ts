/** Priority with abilities to activate on the table, for `?prompt=priority-abilities`: the dev table's look at legal permanents, hand, graveyard, exile and command cards. */
import type { PendingDecision } from '@/play/store'
import type {
  AbilityRef,
  ActivatedView,
  ObjectId,
  ObjectView,
  PlayerView,
  StateView,
} from '@/protocol/generated'
import type { Fixture } from './fixture'
import { creaturesOf } from './effectPrompts'

const ref = (object: ObjectId, ability: number): AbilityRef => ({ object, ability })

/** How the server words cycling in a hand: no cost of its own, the keyword line as the effect. */
const CYCLING: Omit<ActivatedView, 'ability'> = {
  cost: '',
  effect: 'Cycling {2}',
  keyword: 'Cycling {2}',
  unworded: true,
}

/** The viewer's hand cards that are no land, in hand order. */
const spellsOf = (view: StateView, me: PlayerView | undefined): ObjectView[] =>
  (me?.hand.cards ?? []).flatMap((id) => {
    const object = view.objects.find((entry) => entry.id === id)
    return object === undefined || object.types.includes('Land') ? [] : [object]
  })

/**
 * The abilities the scene gives: a creature with one, a creature with two, a graveyard card, the commander, and two
 * hand spells with cycling (the first can also be cast, the last cannot: `priority` leaves it out of `castable`).
 */
function wordsOf(fixture: Fixture): Map<ObjectId, ActivatedView[]> {
  const { view } = fixture
  const me = view.players[view.viewer]
  const spells = spellsOf(view, me)
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
  give(spells.at(0)?.id, CYCLING)
  if (spells.length > 1) give(spells.at(-1)?.id, CYCLING)
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

/** `base` with every ability worded on a card of the table (`abilitiesTable`) offered as one to activate, and the viewer's exiled cards castable from there. */
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
    const exiled = fixture.view.objects
      .filter((object) => object.zone === 'Exile' && object.owner === fixture.view.viewer)
      .map((object) => object.id)
    const castable = [...kind.Priority.castable, ...exiled]
    return {
      ...pending,
      decision: { ...pending.decision, kind: { Priority: { ...kind.Priority, castable, abilities } } },
    }
  }
}

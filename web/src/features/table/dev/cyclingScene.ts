/**
 * A hand card that can be cast or typecycled, for `?prompt=priority-cycling` (the chooser names both with their costs),
 * `?prompt=priority-cycling-only` (the cast is too dear, the Plainscycling is all there is)
 * and `?prompt=pay-cycling` (paying for Plainscycling, which names what it pays for and can be backed out of).
 */
import type { PendingDecision } from '@/play/store'
import type { AbilityRef, ManaOption, ObjectView, PayOptions, StateView } from '@/protocol/generated'
import { withProposal } from './payProposal'
import type { DevPrompt, DevTable } from './prompts'

/** The viewer's first hand card that is no land: the scene dresses it as Angel of the Ruins. */
const cycler = (view: StateView): ObjectView | undefined => {
  const hand = view.players[view.viewer]?.hand.cards ?? []
  return view.objects.find((object) => hand.includes(object.id) && !object.types.includes('Land'))
}

const PLAINSCYCLING = (id: number): AbilityRef => ({ object: id, ability: 1 })

/** Angel of the Ruins as the server shows it in its owner's hand. */
const angel = (object: ObjectView): ObjectView => ({
  ...object,
  name: 'Angel of the Ruins',
  mana_cost: [{ Generic: 5 }, { Colored: 'White' }, { Colored: 'White' }],
  supertypes: [],
  types: ['Artifact', 'Creature'],
  subtypes: ['Angel'],
  colors: ['White'],
  power: 5,
  toughness: 7,
  keywords: ['Flying', 'Plainscycling'],
  rules_text:
    'Flying\nWhen Angel of the Ruins enters, exile up to two target artifacts and/or enchantments.\nPlainscycling {2}',
  activated: [
    {
      ability: PLAINSCYCLING(object.id),
      cost: '{2}, Discard this card',
      effect: 'Search your library for a Plains card, reveal it, put it into your hand, then shuffle.',
      keyword: 'Plainscycling {2}',
    },
  ],
})

/** The table with the viewer's first spell in hand dressed as Angel of the Ruins. */
export const cyclingTable: DevTable = (fixture) => {
  const { view } = fixture
  const card = cycler(view)
  if (card === undefined) return fixture
  const objects = view.objects.map((object) => (object.id === card.id ? angel(object) : object))
  return { ...fixture, view: { ...view, objects } }
}

/** `base`'s priority with Angel of the Ruins castable and its Plainscycling offered. */
export const withCycling =
  (base: DevPrompt): DevPrompt =>
  (fixture) => {
    const pending = base(fixture)
    const card = cycler(fixture.view)
    const { kind } = pending.decision
    if (card === undefined || !('Priority' in kind)) return pending
    const priority = {
      ...kind.Priority,
      castable: [...kind.Priority.castable.filter((id) => id !== card.id), card.id],
      abilities: [...(kind.Priority.abilities ?? []), PLAINSCYCLING(card.id)],
    }
    return { ...pending, decision: { ...pending.decision, kind: { Priority: priority } } }
  }

/** `base`'s priority with Angel of the Ruins too dear to cast and only its Plainscycling offered (`?prompt=priority-cycling-only`). */
export const withCyclingOnly =
  (base: DevPrompt): DevPrompt =>
  (fixture) => {
    const pending = withCycling(base)(fixture)
    const card = cycler(fixture.view)
    const { kind } = pending.decision
    if (card === undefined || !('Priority' in kind)) return pending
    const castable = kind.Priority.castable.filter((id) => id !== card.id)
    return {
      ...pending,
      decision: { ...pending.decision, kind: { Priority: { ...kind.Priority, castable } } },
    }
  }

/** The viewer's untapped basic lands with the one mana each makes. */
const basics = (view: StateView): ManaOption[] =>
  view.objects.flatMap((object): ManaOption[] => {
    if (
      object.zone !== 'Battlefield' ||
      object.controller !== view.viewer ||
      object.permanent?.tapped !== false ||
      !object.supertypes.includes('Basic')
    ) {
      return []
    }
    return object.abilities.flatMap((ability, index): ManaOption[] =>
      'Mana' in ability
        ? [{ source: { object: object.id, ability: index }, produces: ability.Mana.produces }]
        : [],
    )
  })

/** Paying {2} for the Plainscycling of Angel of the Ruins with the viewer's basic lands. */
export const payCycling: DevPrompt = ({ view }): PendingDecision => {
  const card = cycler(view)
  const id = card?.id ?? 0
  const options: PayOptions = withProposal({
    spell: id,
    cost: [{ Generic: 2 }],
    pool: { counts: [0, 0, 0, 0, 0, 0] },
    sources: basics(view),
    auto_available: true,
    can_cancel: true,
    life: view.players[view.viewer]?.life ?? 20,
    max_life: 0,
    ability: PLAINSCYCLING(id),
    spend: { kind: 'Ability', traits: { types: 0, colors: 0 } },
  })
  return { id: 1, decision: { player: view.viewer, kind: { PayMana: options } } }
}

/** Tables for `?prompt=priority-stack-ability` (a triggered ability on the stack), `?prompt=priority-forecast` (a forecast on the top spell) and `?prompt=priority-companion` (a companion at every stage). */
import type { CompanionStage, ObjectView, StateView } from '@/protocol/generated'
import type { Fixture } from './fixture'

const STAGES: readonly CompanionStage[] = ['Hidden', 'Revealed', 'Brought']
const NAMES = ['Lurrus of the Dream-Den', undefined, 'Keruga, the Macrosage']

const withView = (fixture: Fixture, change: Partial<StateView>): Fixture => ({
  ...fixture,
  view: { ...fixture.view, ...change },
})

/** The top spell's card gains a triggered ability above it on the stack, with long rules text and a target. */
export const stackAbilityTable = (fixture: Fixture): Fixture => {
  const { view } = fixture
  const cards = view.stack.cards ?? []
  const source = view.objects.find((o) => o.zone === 'Battlefield' && o.types.includes('Creature'))
  if (source === undefined) return fixture
  const ability: ObjectView = {
    ...structuredClone(source),
    id: Math.max(...view.objects.map((o) => o.id)) + 1,
    zone: 'Stack',
    kind: 'Ability',
    source: source.id,
    types: [],
    colors: [],
    mana_cost: null,
    abilities: [],
    permanent: null,
    power: null,
    toughness: null,
    rules_text:
      'Whenever this creature attacks, target creature an opponent controls gets -2/-2 until end of turn, and you gain life equal to the number of creatures you control.',
  }
  const next = [...cards, ability.id]
  return withView(fixture, {
    objects: [...view.objects, ability],
    stack: { count: next.length, cards: next },
  })
}

/**
 * `?prompt=priority-forecast`: the top spell of the stack is a Giant Growth aimed at a 4/4 of the next seat, with the
 * forecast the engine would give it ("+3/+3 → 7/7" on the creature).
 */
export const forecastStackTable = (fixture: Fixture): Fixture => {
  const { view } = fixture
  const cards = view.stack.cards ?? []
  const top = cards[cards.length - 1]
  const next = (view.viewer + 1) % view.players.length
  const target = view.objects.find(
    (o) => o.zone === 'Battlefield' && o.controller === next && o.types.includes('Creature'),
  )
  if (top === undefined || target === undefined) return fixture
  const objects = view.objects.map((object): ObjectView => {
    if (object.id === target.id) return { ...object, power: 4, toughness: 4 }
    if (object.id !== top) return object
    return {
      ...object,
      name: 'Giant Growth',
      colors: ['Green'],
      rules_text: 'Target creature gets +3/+3 until end of turn.',
      targets: [[{ Object: { id: target.id, counter: 0 } }]],
      targets_legal: [[true]],
      forecast: [{ subject: { Object: target.id }, change: { Stats: { power: [4, 7], toughness: [4, 7] } } }],
    }
  })
  return withView(fixture, { objects })
}

/** Every seat names a companion, one at each stage in turn, the first with a long name. */
export const companionTable = (fixture: Fixture): Fixture =>
  withView(fixture, {
    players: fixture.view.players.map((player, index) => {
      const stage = STAGES[index % STAGES.length] ?? 'Hidden'
      const name = NAMES[index % NAMES.length]
      return { ...player, companion: name === undefined ? { stage } : { stage, name } }
    }),
  })

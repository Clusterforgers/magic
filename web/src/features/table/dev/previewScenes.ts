/** Tables for `?prompt=priority-stack-ability` (a triggered ability on the stack) and `?prompt=priority-companion` (a companion at every stage). */
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

/** Every seat names a companion, one at each stage in turn, the first with a long name. */
export const companionTable = (fixture: Fixture): Fixture =>
  withView(fixture, {
    players: fixture.view.players.map((player, index) => {
      const stage = STAGES[index % STAGES.length] ?? 'Hidden'
      const name = NAMES[index % NAMES.length]
      return { ...player, companion: name === undefined ? { stage } : { stage, name } }
    }),
  })

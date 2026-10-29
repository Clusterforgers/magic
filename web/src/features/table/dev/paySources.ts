/** The payment of the dev table with every kind of mana source, for `?prompt=pay-sources`: a Sol Ring, a Signet, a Treasure, a Command Tower, a bounce land and the fixture's basic lands. */
import type { PendingDecision } from '@/play/store'
import type { ManaCost, ManaOption, ManaType, ObjectId, ObjectView, PayOptions } from '@/protocol/generated'
import type { Fixture } from './fixture'

const COLOURS: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** A permanent added to the viewer's battlefield, with the words of its mana ability and the options it offers. */
interface Source {
  name: string
  types: ObjectView['types']
  subtypes: ObjectView['subtypes']
  cost: string
  effect: string
  /** The options of the ability, for the object with this id. */
  options: (object: ObjectId) => ManaOption[]
}

const one = (object: ObjectId, ability: number, produces: ManaType): ManaOption => ({
  source: { object, ability },
  produces,
})

/** One option for each colour, as a source of any colour offers them. */
const anyColour = (object: ObjectId, extra: Partial<ManaOption> = {}): ManaOption[] =>
  COLOURS.map((colour, ability) => ({ ...one(object, ability, colour), ...extra }))

const SIGNET_COST: ManaCost = [{ Generic: 1 }]

/** The sources the table adds: Sol Ring is the fixture's own when it has an untapped one (its second mana is added to the prompt). */
const ADDED: readonly Source[] = [
  {
    name: 'Orzhov Signet',
    types: ['Artifact'],
    subtypes: [],
    cost: '{1}, {T}',
    effect: 'Add {W}{B}.',
    options: (object) => [{ ...one(object, 0, 'White'), also: ['Black'], cost: SIGNET_COST }],
  },
  {
    name: 'Treasure',
    types: ['Artifact'],
    subtypes: ['Treasure'],
    cost: '{T}, Sacrifice this artifact',
    effect: 'Add one mana of any color.',
    options: (object) => anyColour(object, { sacrifice: true }),
  },
  {
    name: 'Command Tower',
    types: ['Land'],
    subtypes: [],
    cost: '{T}',
    effect: 'Add one mana of any color.',
    options: (object) => anyColour(object),
  },
  {
    name: 'Azorius Chancery',
    types: ['Land'],
    subtypes: [],
    cost: '{T}',
    effect: 'Add {W}{U}.',
    options: (object) => [{ ...one(object, 0, 'White'), also: ['Blue'] }],
  },
  {
    name: 'Sol Ring',
    types: ['Artifact'],
    subtypes: [],
    cost: '{T}',
    effect: 'Add {C}{C}.',
    options: (object) => [{ ...one(object, 0, 'Colorless'), count: 2 }],
  },
]

const FRESH = { tapped: false, damage: 0, summoning_sick: false }

const ownBattlefield = (view: Fixture['view']): ObjectView[] =>
  view.objects.filter((object) => object.zone === 'Battlefield' && object.controller === view.viewer)

/** The viewer's Sol Ring, if the fixture gave them one (the table untaps it). */
const solRing = (view: Fixture['view']): ObjectView | undefined =>
  ownBattlefield(view).find((object) => object.name === 'Sol Ring')

/** The moment: the viewer's permanents are untapped and the sources above are on their battlefield. */
export const paySourcesTable = (fixture: Fixture): Fixture => {
  const { view } = fixture
  const had = solRing(view) !== undefined
  let next = Math.max(...view.objects.map((object) => object.id)) + 1
  const added = ADDED.filter((source) => !(had && source.name === 'Sol Ring')).map((source): ObjectView => {
    const id = next
    next += 1
    return {
      id,
      owner: view.viewer,
      controller: view.viewer,
      zone: 'Battlefield',
      name: source.name,
      mana_cost: null,
      supertypes: [],
      types: source.types,
      subtypes: source.subtypes,
      colors: [],
      power: null,
      toughness: null,
      abilities: [],
      permanent: { ...FRESH },
      activated: [{ cost: source.cost, effect: source.effect, mana: true }],
    }
  })
  const own = new Set(ownBattlefield(view).map((object) => object.id))
  const objects = view.objects.map((object) =>
    own.has(object.id) && object.permanent !== null
      ? { ...object, permanent: { ...object.permanent, tapped: false } }
      : object,
  )
  const cards = [...(view.battlefield.cards ?? []), ...added.map((object) => object.id)]
  return {
    ...fixture,
    view: {
      ...view,
      objects: [...objects, ...added],
      battlefield: { ...view.battlefield, count: cards.length, cards },
    },
  }
}

/** Paying for Selesnya Evangel with the sources of the table: every option of every untapped source of the viewer. */
export const paySources = (fixture: Fixture): PendingDecision => {
  const { view } = fixture
  const viewer = view.viewer
  const spell = view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === viewer)
  // The added sources and two basic lands: a payment search over every land of the fixture is slow to draw.
  const isAdded = (object: ObjectView): boolean => ADDED.some((source) => source.name === object.name)
  const basics = ownBattlefield(view)
    .filter((object) => !isAdded(object))
    .slice(0, 2)
  const offered = ownBattlefield(view).filter((object) => isAdded(object) || basics.includes(object))
  const sources = offered.flatMap((object): ManaOption[] => {
    const added = ADDED.find((source) => source.name === object.name)
    if (added !== undefined) return added.options(object.id)
    return object.abilities.flatMap((ability, index): ManaOption[] =>
      'Mana' in ability ? [one(object.id, index, ability.Mana.produces)] : [],
    )
  })
  const options: PayOptions = {
    spell: spell?.id ?? 0,
    cost: spell?.mana_cost ?? [{ Generic: 2 }, { Colored: 'White' }, { Colored: 'Green' }],
    pool: view.players[viewer]?.mana ?? { counts: [0, 0, 0, 0, 0, 0] },
    sources: sources.filter((option) => {
      const object = view.objects.find((other) => other.id === option.source.object)
      return object?.permanent?.tapped === false
    }),
    auto_available: true,
    can_cancel: true,
    life: view.players[viewer]?.life ?? 20,
    max_life: 0,
  }
  return { id: 1, decision: { player: viewer, kind: { PayMana: options } } }
}

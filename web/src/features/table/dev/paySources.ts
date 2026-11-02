/**
 * The payments of the dev table with every kind of mana source: `?prompt=pay-sources` (a Sol Ring, a Signet, a
 * Treasure, a Command Tower, a bounce land and the fixture's basic lands) and `?prompt=pay-marks` (sources whose chips
 * carry marks: restricted mana, life, energy, another permanent tapped, and green mana kept in the pool to the end of
 * the turn).
 */
import type { PendingDecision } from '@/play/store'
import type {
  ManaCost,
  ManaOption,
  ManaPool,
  ManaType,
  ManaWayView,
  ObjectId,
  ObjectView,
  PayOptions,
} from '@/protocol/generated'
import { madeByOption } from '../manaMade'
import type { Fixture } from './fixture'

const COLOURS: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** A permanent added to the viewer's battlefield, with the words of its mana ability and the options it offers. */
interface Source {
  name: string
  types: ObjectView['types']
  subtypes: ObjectView['subtypes']
  cost: string
  effect: string
  /** The options of the ability, for the object with this id; `creatures` are the viewer's untapped creatures. */
  options: (object: ObjectId, creatures: readonly ObjectId[]) => ManaOption[]
}

const one = (object: ObjectId, ability: number, produces: ManaType): ManaOption => ({
  source: { object, ability },
  produces,
})

/** The way of the view's mana ability (`ActivatedView.ways`) that stands for an option. */
const wayOf = (option: ManaOption): ManaWayView => ({
  mana: madeByOption(option),
  ...(option.cost === undefined ? {} : { cost: option.cost }),
  ...(option.sacrifice === true ? { sacrifice: true } : {}),
})

/** One option for each colour, as a source of any colour offers them. */
const anyColour = (object: ObjectId, extra: Partial<ManaOption> = {}, first = 0): ManaOption[] =>
  COLOURS.map((colour, index) => ({ ...one(object, first + index, colour), ...extra }))

const SIGNET_COST: ManaCost = [{ Generic: 1 }]

/** The sources of `pay-sources`: Sol Ring is the fixture's own when it has an untapped one. */
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

/** The sources of `pay-marks`: each chip carries a mark. */
const MARKED: readonly Source[] = [
  {
    name: 'Ancient Ziggurat',
    types: ['Land'],
    subtypes: [],
    cost: '{T}',
    effect: 'Add one mana of any color. Spend this mana only to cast a creature spell.',
    options: (object) => anyColour(object, { only: [{ Spell: { types: ['Creature'] } }] }),
  },
  {
    name: 'Mana Confluence',
    types: ['Land'],
    subtypes: [],
    cost: '{T}, Pay 1 life',
    effect: 'Add one mana of any color.',
    options: (object) => anyColour(object, { life: 1, extra: true }),
  },
  {
    name: 'Aether Hub',
    types: ['Land'],
    subtypes: [],
    cost: '{T}, Pay {E}',
    effect: 'Add one mana of any color.',
    options: (object) => [
      one(object, 0, 'Colorless'),
      ...anyColour(object, { energy: 1, energy_held: 2 }, 1),
    ],
  },
  {
    name: 'Springleaf Drum',
    types: ['Artifact'],
    subtypes: [],
    cost: '{T}, Tap an untapped creature you control',
    effect: 'Add one mana of any color.',
    options: (object, creatures) => anyColour(object, { takes: [{ count: 1, from: [...creatures] }] }),
  },
]

const FRESH = { tapped: false, damage: 0, summoning_sick: false }

const ownBattlefield = (view: Fixture['view']): ObjectView[] =>
  view.objects.filter((object) => object.zone === 'Battlefield' && object.controller === view.viewer)

const creaturesOf = (view: Fixture['view']): ObjectId[] =>
  ownBattlefield(view)
    .filter((object) => object.types.includes('Creature') && object.permanent?.tapped === false)
    .map((object) => object.id)

/** The moment: the viewer's permanents are untapped and `list` is on their battlefield (the fixture's own Sol Ring kept). */
const tableWith =
  (list: readonly Source[]) =>
  (fixture: Fixture): Fixture => {
    const { view } = fixture
    const own = new Set(ownBattlefield(view).map((object) => object.id))
    const objects = view.objects.map((object) =>
      own.has(object.id) && object.permanent !== null
        ? { ...object, permanent: { ...object.permanent, tapped: false } }
        : object,
    )
    const had = new Set(ownBattlefield(view).map((object) => object.name))
    const creatures = creaturesOf({ ...view, objects })
    let next = Math.max(...view.objects.map((object) => object.id)) + 1
    const added = list
      .filter((source) => !had.has(source.name))
      .map((source): ObjectView => {
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
          activated: [
            {
              cost: source.cost,
              effect: source.effect,
              mana: true,
              ways: source.options(id, creatures).map(wayOf),
            },
          ],
        }
      })
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

const CREATURE_SPELL = 1 << 2

/** Paying for Selesnya Evangel with `list` and two basic lands: every option of every untapped one of them. */
const payWith =
  (list: readonly Source[], pool?: (held: ManaPool) => ManaPool) =>
  (fixture: Fixture): PendingDecision => {
    const { view } = fixture
    const viewer = view.viewer
    const spell = view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === viewer)
    // The listed sources and two basic lands: a payment search over every land of the fixture is slow to draw.
    const listed = (object: ObjectView): Source | undefined =>
      list.find((source) => source.name === object.name)
    const basics = ownBattlefield(view)
      .filter((object) => listed(object) === undefined && object.types.includes('Land'))
      .slice(0, 2)
    const creatures = creaturesOf(view)
    const offered = ownBattlefield(view).filter(
      (object) => listed(object) !== undefined || basics.includes(object),
    )
    const sources = offered.flatMap((object): ManaOption[] => {
      const added = listed(object)
      if (added !== undefined) return added.options(object.id, creatures)
      return object.abilities.flatMap((ability, index): ManaOption[] =>
        'Mana' in ability ? [one(object.id, index, ability.Mana.produces)] : [],
      )
    })
    const held = view.players[viewer]?.mana ?? { counts: [0, 0, 0, 0, 0, 0] }
    const options: PayOptions = {
      spell: spell?.id ?? 0,
      cost: spell?.mana_cost ?? [{ Generic: 2 }, { Colored: 'White' }, { Colored: 'Green' }],
      pool: pool === undefined ? held : pool(held),
      sources: sources.filter((option) => {
        const object = view.objects.find((other) => other.id === option.source.object)
        return object?.permanent?.tapped === false
      }),
      auto_available: true,
      can_cancel: true,
      life: view.players[viewer]?.life ?? 20,
      max_life: 0,
      spend: { kind: 'Spell', traits: { types: CREATURE_SPELL, colors: 2 } },
    }
    return { id: 1, decision: { player: viewer, kind: { PayMana: options } } }
  }

/** One green of the pool lasts to the end of the turn (an Omnath's), so it is marked kept. */
const keptGreen = (held: ManaPool): ManaPool => {
  const counts: ManaPool['counts'] = [...held.counts]
  counts[4] = Math.max(counts[4], 1)
  return { ...held, counts, kept: { combat_end: [0, 0, 0, 0, 0, 0], turn_end: [0, 0, 0, 0, 1, 0] } }
}

/** The table of `pay-sources`. */
export const paySourcesTable = tableWith(ADDED)
/** Paying for Selesnya Evangel with the sources of `pay-sources`. */
export const paySources = payWith(ADDED)
/** The table of `pay-marks`. */
export const payMarksTable = tableWith(MARKED)
/** Paying for Selesnya Evangel with the marked sources, and green kept in the pool. */
export const payMarks = payWith(MARKED, keptGreen)

/**
 * The decisions the Stack column explains, on a board of their own: `?prompt=ask-may` (a "you may" of one trigger),
 * `ask-chain` and `ask-chain-first` (a run of three triggers), `ask-order` (four triggers to order), `ask-pay` and
 * `ask-pay-paid` (a cost with two-colour lands and a Signet) and `ask-departed` (the source has left the battlefield).
 * The viewer's battlefield is a white-blue one: Brago, King Eternal has just entered.
 */
import type { PendingDecision } from '@/play/store'
import type {
  AskingView,
  Color,
  ManaCost,
  ManaOption,
  ManaType,
  ObjectId,
  ObjectView,
  PayOptions,
  RunEntryView,
  Subtype,
  TriggerEntry,
} from '@/protocol/generated'
import { mayClause } from '../prompt/detail/asking'
import type { Fixture } from './fixture'

/** A card of the scene. */
interface Card {
  name: string
  cost: ManaCost
  colors: Color[]
  types: ObjectView['types']
  subtypes?: Subtype[]
  legendary?: boolean
  stats?: [number, number]
  /** Its triggered ability in the card's words. */
  rules?: string
  /** What that ability does, without the event. */
  effect?: string
  /** The question of its "you may". */
  ask?: string
  /** The colours a land or rock makes; two for a source that makes either. */
  makes?: ManaType[]
  /** It makes all of `makes` at once for {1} (a Signet). */
  signet?: boolean
}

const W: ManaCost[number] = { Colored: 'White' }
const U: ManaCost[number] = { Colored: 'Blue' }
const generic = (n: number): ManaCost[number] => ({ Generic: n })

const CARDS: readonly Card[] = [
  {
    name: "Soul's Attendant",
    cost: [W],
    colors: ['White'],
    types: ['Creature'],
    subtypes: ['Human', 'Cleric'],
    stats: [1, 1],
    rules: 'Whenever another creature enters, you may gain 1 life.',
    effect: 'You may gain 1 life.',
    ask: 'Gain 1 life?',
  },
  {
    name: 'Mentor of the Meek',
    cost: [generic(2), W],
    colors: ['White'],
    types: ['Creature'],
    subtypes: ['Human', 'Soldier'],
    stats: [2, 2],
    rules:
      'Whenever another creature you control with power 2 or less enters, you may pay {1}. If you do, draw a card.',
    effect: 'You may pay {1}. If you do, draw a card.',
    ask: 'Pay {1}?',
  },
  {
    name: 'Suture Priest',
    cost: [generic(1), W],
    colors: ['White'],
    types: ['Creature'],
    subtypes: ['Phyrexian', 'Cleric'],
    stats: [1, 1],
    rules: 'Whenever another creature you control enters, you may gain 1 life.',
    effect: 'You may gain 1 life.',
    ask: 'Gain 1 life?',
  },
  {
    name: 'Welcoming Vampire',
    cost: [generic(2), W],
    colors: ['White'],
    types: ['Creature'],
    subtypes: ['Vampire'],
    stats: [2, 3],
    rules:
      'Whenever one or more other creatures you control with power 2 or less enter, draw a card. This ability triggers only once each turn.',
    effect: 'Draw a card.',
  },
  {
    name: 'Brago, King Eternal',
    cost: [generic(2), W, U],
    colors: ['White', 'Blue'],
    types: ['Creature'],
    subtypes: ['Spirit', 'Noble'],
    legendary: true,
    stats: [2, 4],
  },
  { name: 'Plains', cost: [], colors: [], types: ['Land'], subtypes: ['Plains'], makes: ['White'] },
  { name: 'Island', cost: [], colors: [], types: ['Land'], subtypes: ['Island'], makes: ['Blue'] },
  {
    name: 'Hallowed Fountain',
    cost: [],
    colors: [],
    types: ['Land'],
    subtypes: ['Plains', 'Island'],
    makes: ['White', 'Blue'],
  },
  { name: 'Command Tower', cost: [], colors: [], types: ['Land'], makes: ['White', 'Blue'] },
  {
    name: 'Azorius Signet',
    cost: [generic(2)],
    colors: [],
    types: ['Artifact'],
    makes: ['White', 'Blue'],
    signet: true,
  },
]

const SYMBOL: Partial<Record<ManaType, string>> = { White: '{W}', Blue: '{U}' }

/** The view of `card` on the viewer's battlefield. */
function permanent(card: Card, id: ObjectId, viewer: number): ObjectView {
  const makes = card.makes ?? []
  const symbols = makes.map((mana) => SYMBOL[mana] ?? '{C}')
  return {
    id,
    owner: viewer,
    controller: viewer,
    zone: 'Battlefield',
    name: card.name,
    mana_cost: card.cost.length === 0 ? null : card.cost,
    supertypes:
      card.legendary === true
        ? ['Legendary']
        : card.types.includes('Land') && makes.length === 1
          ? ['Basic']
          : [],
    types: card.types,
    subtypes: card.subtypes ?? [],
    colors: card.colors,
    power: card.stats?.[0] ?? null,
    toughness: card.stats?.[1] ?? null,
    abilities: makes.length === 1 && makes[0] !== undefined ? [{ Mana: { produces: makes[0] } }] : [],
    permanent: { tapped: false, damage: 0, summoning_sick: false },
    ...(makes.length < 2
      ? {}
      : {
          activated: [
            {
              cost: card.signet === true ? '{1}, {T}' : '{T}',
              effect: card.signet === true ? `Add ${symbols.join('')}.` : `Add ${symbols.join(' or ')}.`,
              mana: true,
              ways:
                card.signet === true
                  ? [{ mana: makes, cost: [generic(1)] }]
                  : makes.map((mana) => ({ mana: [mana] })),
            },
          ],
        }),
  }
}

/** The scene: the fixture with the viewer's battlefield replaced by `CARDS`, on its own main phase with no combat. */
interface Scene {
  fixture: Fixture
  /** The ids of the scene's cards, by name. */
  ids: Readonly<Record<string, ObjectId>>
}

/** What the scene puts on the stack: the triggers of these cards, the last on top; or Brago as a spell. */
type Stacked = { triggers: readonly string[] } | { spell: true }

function scene(fixture: Fixture, stacked: Stacked): Scene {
  const { view } = fixture
  const viewer = view.viewer
  const kept = view.objects.filter(
    (object) => !(object.controller === viewer && object.zone === 'Battlefield') && object.zone !== 'Stack',
  )
  let next = Math.max(...view.objects.map((object) => object.id)) + 1
  const ids: Record<string, ObjectId> = {}
  const spell = 'spell' in stacked
  const own = CARDS.map((card): ObjectView => {
    const id = next
    next += 1
    ids[card.name] = id
    const object = permanent(card, id, viewer)
    return spell && card.name === 'Brago, King Eternal'
      ? { ...object, zone: 'Stack', permanent: null }
      : object
  })
  const abilities = ('triggers' in stacked ? stacked.triggers : []).map((name): ObjectView => {
    const card = CARDS.find((one) => one.name === name)
    const source = own.find((object) => object.name === name)
    const id = next
    next += 1
    ids[`${name} trigger`] = id
    return {
      id,
      owner: viewer,
      controller: viewer,
      zone: 'Stack',
      name,
      mana_cost: null,
      supertypes: [],
      types: [],
      subtypes: [],
      colors: [],
      power: null,
      toughness: null,
      abilities: [],
      permanent: null,
      kind: 'Ability',
      ...(source === undefined ? {} : { source: source.id }),
      ...(card?.rules === undefined ? {} : { rules_text: card.rules }),
    }
  })
  const field = own.filter((object) => object.zone === 'Battlefield').map((object) => object.id)
  const others = (view.battlefield.cards ?? []).filter((id) => kept.some((object) => object.id === id))
  const stack = spell ? [ids['Brago, King Eternal'] ?? 0] : abilities.map((object) => object.id)
  return {
    ids,
    fixture: {
      ...fixture,
      view: {
        ...view,
        objects: [...kept, ...own, ...abilities],
        battlefield: {
          ...view.battlefield,
          count: others.length + field.length,
          cards: [...others, ...field],
        },
        stack: { count: stack.length, cards: stack },
        turn: { ...view.turn, active: viewer, step: 'Main1' },
        combat: { attackers: [], attackers_declared: false, blockers_declared: [] },
        combat_forecast: null,
        priority: { holder: viewer, passed: [] },
      },
    },
  }
}

/** The ids of the scene's cards as the table already holds them. */
function idsOf(fixture: Fixture): Record<string, ObjectId> {
  const ids: Record<string, ObjectId> = {}
  for (const object of fixture.view.objects) {
    if (object.controller !== fixture.view.viewer) continue
    if (object.kind === 'Ability') ids[`${object.name} trigger`] = object.id
    else if (CARDS.some((card) => card.name === object.name)) ids[object.name] = object.id
  }
  return ids
}

const cardOf = (name: string): Card =>
  CARDS.find((card) => card.name === name) ?? { name, cost: [], colors: [], types: [] }

/** The question a trigger of `name` asks, with what asks it. */
function mayOf(
  fixture: Fixture,
  name: string,
  run?: { index: number; order: readonly string[] },
): PendingDecision {
  const ids = idsOf(fixture)
  const card = cardOf(name)
  const text = card.rules ?? ''
  const clause = mayClause(text, card.ask ?? '')
  const brago = ids['Brago, King Eternal']
  const entries = run?.order.map((member, place): RunEntryView => {
    const done = place < run.index - 1
    const source = ids[member]
    const object = ids[`${member} trigger`]
    const effect = cardOf(member).effect
    return {
      name: member,
      ...(source === undefined ? {} : { source }),
      ...(done || object === undefined ? {} : { object }),
      ...(done || effect === undefined ? {} : { text: effect }),
      ...(done ? { done: true } : {}),
    }
  })
  const asking: AskingView = {
    name,
    ability: 0,
    text,
    ...(ids[`${name} trigger`] === undefined ? {} : { stack: ids[`${name} trigger`], resolving: true }),
    ...(ids[name] === undefined ? {} : { source: ids[name] }),
    ...(clause === undefined ? {} : { clause: [clause[0], clause[1]] }),
    ...(brago === undefined ? {} : { cause: { object: brago, name: 'Brago, King Eternal', entered: true } }),
    ...(run === undefined || entries === undefined ? {} : { run: { id: 7, index: run.index, entries } }),
  }
  return {
    id: 1,
    decision: {
      player: fixture.view.viewer,
      kind: { YesNo: { source: ids[name] ?? 0, text: card.ask ?? '' } },
    },
    asking,
  }
}

const CHAIN = ["Soul's Attendant", 'Suture Priest', 'Mentor of the Meek'] as const
const ORDER = ['Welcoming Vampire', 'Mentor of the Meek', "Soul's Attendant", 'Suture Priest'] as const

/** Four triggers of Brago entering, to put in order. */
function orderOf(fixture: Fixture): PendingDecision {
  const ids = idsOf(fixture)
  const brago = ids['Brago, King Eternal']
  const triggers = ORDER.map((name): TriggerEntry => {
    const effect = cardOf(name).effect
    return {
      source: { id: ids[name] ?? 0, counter: 0 },
      ability: 0,
      when: 'Whenever another creature enters',
      ...(effect === undefined ? {} : { text: effect }),
      ...(brago === undefined ? {} : { cause: brago }),
    }
  })
  return { id: 1, decision: { player: fixture.view.viewer, kind: { OrderTriggers: { triggers } } } }
}

/** Paying {2}{W}{U} for Brago with a Plains, an Island, two lands of either colour and a Signet. */
function payOf(fixture: Fixture): PendingDecision {
  const { view } = fixture
  const ids = idsOf(fixture)
  const sources = CARDS.flatMap((card): ManaOption[] => {
    const object = ids[card.name]
    const makes = card.makes
    if (object === undefined || makes === undefined) return []
    const [first, ...rest] = makes
    if (first === undefined) return []
    if (card.signet === true) {
      return [{ source: { object, ability: 0 }, produces: first, also: rest, cost: [generic(1)] }]
    }
    return makes.map((produces, ability) => ({ source: { object, ability }, produces }))
  })
  const spell = ids['Brago, King Eternal'] ?? 0
  const options: PayOptions = {
    spell,
    cost: cardOf('Brago, King Eternal').cost,
    pool: { counts: [0, 0, 0, 0, 0, 0] },
    sources,
    auto_available: true,
    can_cancel: true,
    life: view.players[view.viewer]?.life ?? 40,
    max_life: 0,
    spend: { kind: 'Spell', traits: { types: 1 << 2, colors: 3 } },
  }
  return {
    id: 1,
    decision: { player: view.viewer, kind: { PayMana: options } },
    asking: { stack: spell, source: spell, name: 'Brago, King Eternal' },
  }
}

/** The tables of the scene, by `?prompt=` name. */
export const ASK_TABLES: Record<string, (fixture: Fixture) => Fixture> = {
  'ask-may': (fixture) => scene(fixture, { triggers: ['Mentor of the Meek'] }).fixture,
  'ask-chain-first': (fixture) => scene(fixture, { triggers: [...CHAIN].reverse() }).fixture,
  // The first of the three has resolved: two are left, the asking one on top.
  'ask-chain': (fixture) => scene(fixture, { triggers: [...CHAIN.slice(1)].reverse() }).fixture,
  'ask-order': (fixture) => scene(fixture, { triggers: [] }).fixture,
  'ask-pay': (fixture) => scene(fixture, { spell: true }).fixture,
  // The source died in response: its ability still asks, by name alone.
  'ask-departed': (fixture) => {
    const made = scene(fixture, { triggers: ['Mentor of the Meek'] })
    const gone = made.ids['Mentor of the Meek']
    const { view } = made.fixture
    return {
      ...made.fixture,
      view: {
        ...view,
        objects: view.objects.filter((object) => object.id !== gone),
        battlefield: {
          ...view.battlefield,
          count: view.battlefield.count - 1,
          cards: (view.battlefield.cards ?? []).filter((id) => id !== gone),
        },
      },
    }
  },
}

/** The decisions of the scene, by `?prompt=` name. */
export const ASK_PROMPTS: Record<string, (fixture: Fixture) => PendingDecision> = {
  'ask-may': (fixture) => mayOf(fixture, 'Mentor of the Meek'),
  'ask-chain-first': (fixture) => mayOf(fixture, "Soul's Attendant", { index: 1, order: CHAIN }),
  'ask-chain': (fixture) => mayOf(fixture, 'Suture Priest', { index: 2, order: CHAIN }),
  'ask-order': orderOf,
  'ask-pay': payOf,
  'ask-departed': (fixture) => {
    const pending = mayOf(fixture, 'Mentor of the Meek')
    const { source: _gone, ...asking } = pending.asking ?? {}
    return { ...pending, asking }
  },
}

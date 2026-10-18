/** Decisions the dev table can show: `/dev/table?prompt=<name>`. Add a line to `DEV_PROMPTS` for each new prompt. */
import type { PendingDecision } from '@/play/store'
import type {
  AttackOptions,
  AttackTarget,
  BlockOptions,
  DecisionKind,
  ManaCost,
  ManaOption,
  ObjectId,
  ObjectView,
  PayOptions,
  PlayerId,
  PriorityOptions,
  StateView,
} from '@/protocol/generated'
import { assignDamage } from './damagePrompt'
import type { Fixture } from './fixture'

/** Builds the decision the viewer of a fixture faces. */
export type DevPrompt = (fixture: Fixture) => PendingDecision

const isLand = (object: ObjectView): boolean => object.types.includes('Land')

/**
 * Priority over the fixture's stack: the hand's lands may be played, all but the last spell of the hand may be cast
 * and every untapped permanent that makes mana may be tapped. The first non-land mana source gets an invented second
 * ability, since the fixture has none with two, so the choice between abilities can be seen.
 */
const priority: DevPrompt = ({ view }) => {
  const viewer = view.viewer
  const hand = view.objects.filter((object) => view.players[viewer]?.hand.cards?.includes(object.id))
  const spells = hand.filter((object) => !isLand(object))
  const sources = view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === viewer &&
      object.permanent?.tapped === false &&
      object.abilities.length > 0,
  )
  const mana_abilities = sources.flatMap((object): ManaOption[] => {
    const own = object.abilities.flatMap((ability, index): ManaOption[] =>
      'Mana' in ability
        ? [{ source: { object: object.id, ability: index }, produces: ability.Mana.produces }]
        : [],
    )
    const [first] = own
    const invented = !isLand(object) && object === sources.find((source) => !isLand(source))
    return invented && first !== undefined
      ? [...own, { source: { object: object.id, ability: first.source.ability + 1 }, produces: 'Red' }]
      : own
  })
  const options: PriorityOptions = {
    playable_lands: hand.filter(isLand).map((object) => object.id),
    castable: spells.slice(0, -1).map((object) => object.id),
    mana_abilities,
    can_pass: true,
  }
  return { id: 1, decision: { player: viewer, kind: { Priority: options } } }
}

const untappedCreatures = (view: StateView, seat: PlayerId): ObjectView[] =>
  view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === seat &&
      object.types.includes('Creature') &&
      object.permanent?.tapped === false,
  )

/** Every seat but `seat` that is still in, in turn order after it. */
const othersAfter = (view: StateView, seat: PlayerId): PlayerId[] =>
  view.players
    .filter((player) => player.seat !== seat && player.lost === null)
    .map((player) => player.seat)
    .sort((a, b) => ((a - seat + 99) % 99) - ((b - seat + 99) % 99))

/**
 * The viewer declares attackers: every untapped creature that is not summoning sick may attack any other seat, or the
 * planeswalker `attackTable` put in play.
 */
const attack: DevPrompt = ({ view }) => {
  const viewer = view.viewer
  const defenders: AttackTarget[] = [
    ...othersAfter(view, viewer).map((seat): AttackTarget => ({ Player: seat })),
    ...view.objects
      .filter((object) => object.types.includes('Planeswalker') && object.controller !== viewer)
      .map((object): AttackTarget => ({ Planeswalker: object.id })),
  ]
  const options: AttackOptions = {
    attackers: untappedCreatures(view, viewer)
      .filter((object) => object.permanent?.summoning_sick === false)
      .map((object) => ({ object: object.id, defenders })),
  }
  return { id: 1, decision: { player: viewer, kind: { DeclareAttackers: options } } }
}

/** The viewer declares blockers: every untapped creature may block each attacker that is aimed at the viewer. */
const block: DevPrompt = ({ view }) => {
  const viewer = view.viewer
  const can_block = view.combat.attackers
    .filter((attacker) => 'Player' in attacker.target && attacker.target.Player === viewer)
    .map((attacker) => attacker.object)
  const options: BlockOptions = {
    blockers: untappedCreatures(view, viewer).map((object) => ({ object: object.id, can_block })),
  }
  return { id: 1, decision: { player: viewer, kind: { DeclareBlockers: options } } }
}

/** A table for the moment a decision belongs to; the default fixture is in the middle of an attack. */
export type DevTable = (fixture: Fixture) => Fixture

const withView = (fixture: Fixture, change: Partial<StateView>): Fixture => ({
  ...fixture,
  view: { ...fixture.view, ...change },
})

/** The same objects with `ids` tapped, as attacking taps. */
const tapped = (objects: readonly ObjectView[], ids: ReadonlySet<ObjectId>): ObjectView[] =>
  objects.map((object) =>
    ids.has(object.id) && object.permanent !== null
      ? { ...object, permanent: { ...object.permanent, tapped: true } }
      : object,
  )

/** The turn is the viewer's and nobody has attacked yet; one of the first opponent's permanents is a planeswalker. */
const attackTable: DevTable = (fixture) => {
  const { view } = fixture
  const owner = othersAfter(view, view.viewer)[0] ?? view.viewer
  const walker: ObjectView = {
    id: Math.max(...view.objects.map((object) => object.id)) + 1,
    owner,
    controller: owner,
    zone: 'Battlefield',
    name: 'Jace, the Mind Sculptor',
    mana_cost: [{ Generic: 2 }, { Colored: 'Blue' }, { Colored: 'Blue' }],
    supertypes: ['Legendary'],
    types: ['Planeswalker'],
    subtypes: [],
    colors: ['Blue'],
    power: null,
    toughness: null,
    abilities: [],
    permanent: { tapped: false, damage: 0, summoning_sick: false },
  }
  const cards = [...(view.battlefield.cards ?? []), walker.id]
  return withView(fixture, {
    objects: [...view.objects, walker],
    battlefield: { count: cards.length, cards },
    turn: { ...view.turn, active: view.viewer, step: 'DeclareAttackers' },
    priority: null,
    combat: { attackers: [], attackers_declared: false, blockers_declared: [] },
  })
}

/**
 * The active seat has attacked with six creatures and nobody has blocked: four at the viewer and, when there is a
 * third seat, two at another player (which the viewer may not block).
 */
const blockTable: DevTable = (fixture) => {
  const { view } = fixture
  const active = view.turn.active
  const elsewhere = othersAfter(view, view.viewer).find((seat) => seat !== active)
  const declared = new Set(view.combat.attackers.map((attacker) => attacker.object))
  const more = untappedCreatures(view, active)
    .filter((object) => object.permanent?.summoning_sick === false && !declared.has(object.id))
    .map((object) => object.id)
  const ids = [...declared, ...more].slice(0, 6)
  const attackers = ids.map((object, index) => ({
    object,
    target: { Player: index < 4 || elsewhere === undefined ? view.viewer : elsewhere },
    blocked: false,
    blockers: [],
  }))
  return withView(fixture, {
    objects: tapped(view.objects, new Set(ids)),
    combat: { attackers, attackers_declared: true, blockers_declared: [] },
    priority: null,
  })
}

/**
 * Paying for Selesnya Evangel ({2}{W}{G}) from the hand, with the viewer's floating mana (two red, a green, a
 * colourless), every untapped land and mana rock, and one invented second ability (red) on the first rock so the
 * choice between abilities can be seen. The other pay prompts show hybrid and Phyrexian symbols.
 */
const pay: DevPrompt = ({ view }) => {
  const viewer = view.viewer
  const spell = view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === viewer)
  const sources = view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === viewer &&
      object.permanent?.tapped === false &&
      object.abilities.length > 0,
  )
  const rock = sources.find((object) => !object.types.includes('Land'))
  const options: PayOptions = {
    spell: spell?.id ?? 0,
    cost: spell?.mana_cost ?? [{ Generic: 2 }, { Colored: 'White' }, { Colored: 'Green' }],
    pool: view.players[viewer]?.mana ?? { counts: [0, 0, 0, 0, 0, 0] },
    sources: sources.flatMap((object): ManaOption[] =>
      object.abilities.flatMap((ability, index): ManaOption[] =>
        'Mana' in ability
          ? [{ source: { object: object.id, ability: index }, produces: ability.Mana.produces }]
          : [],
      ),
    ),
    auto_available: true,
    can_cancel: true,
    life: view.players[viewer]?.life ?? 20,
    max_life: 0,
  }
  if (rock !== undefined) {
    options.sources.push({ source: { object: rock.id, ability: rock.abilities.length }, produces: 'Red' })
  }
  return { id: 1, decision: { player: viewer, kind: { PayMana: options } } }
}

/** A cost the viewer cannot pay (two blue, with three sources and none of them blue): no automatic payment. */
const payShort: DevPrompt = (fixture) => {
  const { id, decision } = pay(fixture)
  if (!('PayMana' in decision.kind)) return { id, decision }
  const { PayMana: options } = decision.kind
  const short: PayOptions = {
    ...options,
    cost: [{ Generic: 3 }, { Colored: 'Blue' }, { Colored: 'Blue' }],
    pool: { counts: [0, 0, 0, 0, 0, 0] },
    sources: options.sources.filter((option) => option.produces !== 'Blue').slice(0, 3),
    auto_available: false,
  }
  return { id, decision: { ...decision, kind: { PayMana: short } } }
}

const HYBRID_COST: ManaCost = [
  { Generic: 1 },
  { Hybrid: ['Green', 'White'] },
  { MonoHybrid: 'Blue' },
  { Phyrexian: 'White' },
  { HybridPhyrexian: ['Green', 'Blue'] },
]

/**
 * Hybrid, monocolored hybrid and Phyrexian symbols in one cost, `{1}{G/W}{2/U}{W/P}{G/U/P}`, with 20 life: two
 * symbols that life can pay, each with a switch, and the lands to pay the rest from.
 */
const payHybrid: DevPrompt = (fixture) => {
  const { id, decision } = pay(fixture)
  if (!('PayMana' in decision.kind)) return { id, decision }
  const { PayMana: options } = decision.kind
  const mixed: PayOptions = { ...options, cost: HYBRID_COST, life: 20, max_life: 4 }
  return { id, decision: { ...decision, kind: { PayMana: mixed } } }
}

/** The spell being paid for gets the hybrid cost too, so its card in the hand draws the symbols. */
const hybridTable: DevTable = (fixture) => {
  const { objects, viewer } = fixture.view
  const spell = objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === viewer)
  return withView(fixture, {
    objects: objects.map((object) =>
      object.id === spell?.id ? { ...object, mana_cost: HYBRID_COST } : object,
    ),
  })
}

/** The same cost when the viewer has 3 life: 2 can be paid for one symbol, not for both, and the switch for the second is dead. */
const payLowLife: DevPrompt = (fixture) => {
  const { id, decision } = payHybrid(fixture)
  if (!('PayMana' in decision.kind)) return { id, decision }
  const low: PayOptions = { ...decision.kind.PayMana, life: 3, max_life: 2 }
  return { id, decision: { ...decision, kind: { PayMana: low } } }
}

/** The same payment when the cast may not be backed out of: no Cancel button, and the sentence does not mention it. */
const payLocked: DevPrompt = (fixture) => {
  const { id, decision } = pay(fixture)
  if (!('PayMana' in decision.kind)) return { id, decision }
  const locked: PayOptions = { ...decision.kind.PayMana, can_cancel: false }
  return { id, decision: { ...decision, kind: { PayMana: locked } } }
}

/** A decision about the viewer's hand, such as an opening hand to judge or cards to discard. */
const aboutHand =
  (kind: (hand: ObjectId[]) => DecisionKind): DevPrompt =>
  ({ view }) => ({
    id: 1,
    decision: { player: view.viewer, kind: kind(view.players[view.viewer]?.hand.cards ?? []) },
  })

/** The prompts by their `?prompt=` name. */
export const DEV_PROMPTS: Record<string, DevPrompt> = {
  priority,
  attack,
  block,
  mulligan: aboutHand((hand) => ({
    Mulligan: {
      hand,
      mulligans_taken: 1,
      can_mulligan: true,
      free_mulligans: 0,
      bottom_if_keep: 1,
      bottom_if_mulligan: 2,
    },
  })),
  // A format with one free mulligan (Commander's, CR 103.5c): the next mulligan costs no card.
  'mulligan-free': aboutHand((hand) => ({
    Mulligan: {
      hand,
      mulligans_taken: 0,
      can_mulligan: true,
      free_mulligans: 1,
      bottom_if_keep: 0,
      bottom_if_mulligan: 0,
    },
  })),
  bottom: aboutHand((hand) => ({ BottomCards: { hand, count: 2 } })),
  discard: aboutHand((hand) => ({ Discard: { hand: hand.slice(0, -1), count: 2 } })),
  damage: assignDamage(2),
  damage3: assignDamage(3),
  damage1: assignDamage(1),
  pay,
  'pay-short': payShort,
  'pay-locked': payLocked,
  'pay-hybrid': payHybrid,
  'pay-low-life': payLowLife,
}

/** Prompts that need another moment of the game than the default fixture shows, by `?prompt=` name. */
export const DEV_TABLES: Record<string, DevTable> = {
  attack: attackTable,
  block: blockTable,
  'pay-hybrid': hybridTable,
  'pay-low-life': hybridTable,
}

/**
 * A realistic table for the dev route and the tests: every seat has about 40 permanents (lands, creatures, artifacts
 * and enchantments, some tapped, summoning sick or damaged), a graveyard and a commander; the viewer has a hand; an
 * attack is under way with blockers; three spells are on the stack and there are about 50 log events.
 */
import type { ChatLine, LogEntry } from '@/play/store'
import type {
  CombatView,
  ManaPool,
  ObjectId,
  ObjectView,
  PlayerId,
  PlayerView,
  StateView,
  Zone,
  ZoneView,
} from '@/protocol/generated'
import { COMMANDERS, CREATURES, INSTANTS, LANDS, OTHERS } from './cards'
import type { NonEmpty, Template } from './cards'
import { buildLog } from './fixtureLog'

/** The text the dev page prints, so a production bundle can be searched for any trace of the fixture. */
export const FIXTURE_MARKER = 'dev-table-fixture-8e1c'

/** The names the fixture gives its seats, in seat order. */
export const NAMES = ['Alice', 'Bo', 'Cy', 'Dee', 'Eli', 'Fay', 'Gus', 'Hana', 'Ivo', 'Jun', 'Kit', 'Lena']

/** What to build. */
export interface FixtureOptions {
  /** 2 to 12. */
  seats: number
  /** The seat that looks at the table; 0 by default. */
  viewer?: PlayerId
  /** Permanents per seat; 40 by default. */
  permanents?: number
  /** The seat whose turn it is and who attacks; the seat after the viewer by default. */
  active?: PlayerId
}

/** The table, its log and chat, and the seat names. */
export interface Fixture {
  view: StateView
  log: LogEntry[]
  chat: ChatLine[]
  names: string[]
}

/** The ids of the cards one seat owns, by place. */
export interface SeatCards {
  lands: ObjectId[]
  creatures: ObjectId[]
  others: ObjectId[]
  graveyard: ObjectId[]
  hand: ObjectId[]
  command: ObjectId[]
}

/** What `buildLog` needs to name real cards. */
export interface LogContext {
  names: string[]
  viewer: PlayerId
  active: PlayerId
  defender: PlayerId
  cards: SeatCards[]
  attackers: ObjectId[]
  blockers: ObjectId[]
  stack: ObjectId[]
}

const zoneView = (cards: ObjectId[]): ZoneView => ({ count: cards.length, cards })
const hidden = (count: number): ZoneView => ({ count, cards: null })
const NO_MANA: ManaPool = { counts: [0, 0, 0, 0, 0, 0] }

/** Builds a table of `seats` seats; the same options always give the same table. */
export function buildFixture(options: FixtureOptions): Fixture {
  const seats = Math.min(12, Math.max(2, Math.floor(options.seats)))
  const viewer = options.viewer ?? 0
  const active = options.active ?? (viewer + 1) % seats
  const defender = active === viewer ? (viewer + 1) % seats : viewer
  const perSeat = options.permanents ?? 40
  const landCount = Math.round(perSeat * 0.4)
  const creatureCount = Math.round(perSeat * 0.35)
  const otherCount = Math.max(0, perSeat - landCount - creatureCount)

  const objects: ObjectView[] = []
  let nextId = 1
  const add = (template: Template, owner: PlayerId, zone: Zone, controller: PlayerId = owner): ObjectView => {
    const permanent = zone === 'Battlefield' ? { tapped: false, damage: 0, summoning_sick: false } : null
    const object: ObjectView = {
      id: nextId,
      owner,
      controller,
      zone,
      name: template.name,
      mana_cost: template.cost,
      supertypes: template.supertypes ?? [],
      types: template.types,
      subtypes: template.subtypes ?? [],
      colors: template.colors,
      power: template.power ?? null,
      toughness: template.toughness ?? null,
      abilities: template.produces === undefined ? [] : [{ Mana: { produces: template.produces } }],
      permanent,
    }
    nextId += 1
    objects.push(object)
    return object
  }
  const edit = (id: ObjectId, change: (object: ObjectView) => void): void => {
    const object = objects.find((candidate) => candidate.id === id)
    if (object !== undefined) change(object)
  }
  const pick = <T>(list: NonEmpty<T>, index: number): T => list[index % list.length] ?? list[0]

  const cards: SeatCards[] = []
  for (let seat = 0; seat < seats; seat += 1) {
    const mine: SeatCards = { lands: [], creatures: [], others: [], graveyard: [], hand: [], command: [] }
    for (let i = 0; i < landCount; i += 1) {
      const land = add(pick(LANDS, i + seat), seat, 'Battlefield')
      if (i % 3 === 0 && land.permanent !== null) land.permanent.tapped = true
      mine.lands.push(land.id)
    }
    for (let i = 0; i < creatureCount; i += 1) {
      const creature = add(pick(CREATURES, i + 2 * seat), seat, 'Battlefield')
      if (creature.permanent !== null) {
        if (i % 5 === 1) creature.permanent.tapped = true
        else if (i >= creatureCount - 2) creature.permanent.summoning_sick = true
        if (i % 6 === 3) creature.permanent.damage = 1
      }
      mine.creatures.push(creature.id)
    }
    for (let i = 0; i < otherCount; i += 1) {
      const other = add(pick(OTHERS, i + seat), seat, 'Battlefield')
      if (i % 4 === 0 && other.permanent !== null && other.abilities.length > 0) other.permanent.tapped = true
      mine.others.push(other.id)
    }
    const yard: Template[] = [
      pick(CREATURES, seat),
      pick(INSTANTS, seat),
      pick(CREATURES, seat + 5),
      pick(INSTANTS, seat + 2),
      pick(LANDS, seat),
    ]
    for (const template of yard) mine.graveyard.push(add(template, seat, { Graveyard: seat }).id)
    mine.command.push(add(pick(COMMANDERS, seat), seat, { Command: seat }).id)
    if (seat === viewer) {
      const hand: Template[] = [
        pick(LANDS, 1),
        pick(LANDS, 3),
        pick(CREATURES, 4),
        pick(CREATURES, 9),
        pick(CREATURES, 12),
        pick(INSTANTS, 0),
        pick(INSTANTS, 4),
      ]
      for (const template of hand) mine.hand.push(add(template, seat, { Hand: seat }).id)
    }
    cards.push(mine)
  }

  // Combat: the active seat attacks with three untapped creatures; the defender double-blocks the first.
  const attackers = (cards[active]?.creatures ?? [])
    .filter((id) => objects.find((o) => o.id === id)?.permanent?.tapped === false)
    .slice(0, 3)
  const blockers = (cards[defender]?.creatures ?? []).slice(0, 2)
  for (const id of attackers) {
    edit(id, (object) => {
      if (object.permanent !== null) object.permanent.tapped = true
    })
  }
  const combat: CombatView = {
    attackers: attackers.map((object, index) => ({
      object,
      target: { Player: defender },
      blocked: index === 0,
      blockers: index === 0 ? blockers : [],
    })),
    attackers_declared: true,
    blockers_declared: [defender],
  }

  const stackSpells: [Template, PlayerId][] = [
    [pick(INSTANTS, 1), (viewer + 2) % seats],
    [pick(INSTANTS, 2), active],
    [pick(INSTANTS, 0), viewer],
  ]
  const stack = stackSpells.map(([template, caster]) => add(template, caster, 'Stack').id)
  const exile = [viewer, active, (viewer + 2) % seats].map(
    (owner, index) => add(pick(INSTANTS, index + 3), owner, 'Exile').id,
  )

  // The shared battlefield lists one seat's permanent after another's, as entering the game over time would.
  const lanes = cards.map((mine) => [...mine.lands, ...mine.creatures, ...mine.others])
  const battlefield: ObjectId[] = []
  for (let i = 0; i < Math.max(...lanes.map((lane) => lane.length)); i += 1) {
    for (const lane of lanes) {
      const id = lane[i]
      if (id !== undefined) battlefield.push(id)
    }
  }

  const players: PlayerView[] = cards.map((mine, seat) => ({
    seat,
    life: 40 - 2 * seat,
    lost: null,
    mana:
      seat === viewer
        ? { counts: [0, 0, 0, 2, 1, 1] }
        : seat === active
          ? { counts: [1, 0, 0, 0, 0, 0] }
          : NO_MANA,
    mulligans: 0,
    mulligan: 'Kept',
    hand: seat === viewer ? zoneView(mine.hand) : hidden(5 + (seat % 3)),
    library: hidden(48 - seat),
    graveyard: zoneView(mine.graveyard),
    command: zoneView(mine.command),
  }))

  const view: StateView = {
    objects: [...objects].sort((a, b) => a.id - b.id),
    viewer,
    format: 'Commander',
    stage: 'Turns',
    first_player: 0,
    turn: { number: 5, active, step: 'DeclareBlockers', lands_played: 1 },
    priority: { holder: viewer, passed: [active] },
    players,
    battlefield: zoneView(battlefield),
    stack: zoneView(stack),
    exile: zoneView(exile),
    combat,
    result: null,
  }
  const names = NAMES.slice(0, seats)
  const log = buildLog({ names, viewer, active, defender, cards, attackers, blockers, stack })
  const chat: ChatLine[] = [
    { from: active, text: 'Attacking with the whole squad.' },
    { from: viewer, text: 'I have a response.' },
    { from: (viewer + 2) % seats, text: 'Counterspell on the bolt, if it comes to that.' },
  ]
  return { view, log, chat, names }
}

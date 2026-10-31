/**
 * The Rooms scene of `/dev/table?prompt=priority-rooms&turn=you`: the viewer's main phase with three Rooms of its own (both
 * doors locked, one door unlocked, both unlocked) and one of a rival's, the viewer offered each of its locked doors to
 * unlock, and the card endpoint answering with the real halves of the four Rooms.
 */
import type { PendingDecision } from '@/play/store'
import type {
  CardDetail,
  CardFaceView,
  FaceOption,
  ManaCost,
  ObjectId,
  ObjectView,
  PlayerId,
  PriorityOptions,
} from '@/protocol/generated'
import type { CardsApi } from '@/net/cards'
import { costText } from '../cardModel'
import type { Fixture } from './fixture'

/** One half of a Room: its name, cost and rules text as the card prints them (the reminder text left out). */
interface Half {
  name: string
  cost: ManaCost
  text: string
}

interface RoomSpec {
  oracle: string
  halves: [Half, Half]
  unlocked: [boolean, boolean]
  color: 'White' | 'Blue' | 'Green'
  owner: 'viewer' | 'rival'
}

const W = { Colored: 'White' } as const
const U = { Colored: 'Blue' } as const
const G = { Colored: 'Green' } as const

const SPECS: RoomSpec[] = [
  {
    oracle: 'room-surgical-suite',
    halves: [
      {
        name: 'Surgical Suite',
        cost: [{ Generic: 1 }, W],
        text: 'When you unlock this door, return target creature card with mana value 3 or less from your graveyard to the battlefield.',
      },
      {
        name: 'Hospital Room',
        cost: [{ Generic: 3 }, W],
        text: 'Whenever you attack, put a +1/+1 counter on target attacking creature.',
      },
    ],
    unlocked: [false, false],
    color: 'White',
    owner: 'viewer',
  },
  {
    oracle: 'room-bottomless-pool',
    halves: [
      {
        name: 'Bottomless Pool',
        cost: [U],
        text: "When you unlock this door, return up to one target creature to its owner's hand.",
      },
      {
        name: 'Locker Room',
        cost: [{ Generic: 4 }, U],
        text: 'Whenever one or more creatures you control deal combat damage to a player, draw a card.',
      },
    ],
    unlocked: [true, false],
    color: 'Blue',
    owner: 'viewer',
  },
  {
    oracle: 'room-grand-entryway',
    halves: [
      {
        name: 'Grand Entryway',
        cost: [{ Generic: 1 }, W],
        text: 'When you unlock this door, create a 1/1 white Glimmer enchantment creature token.',
      },
      {
        name: 'Elegant Rotunda',
        cost: [{ Generic: 2 }, W],
        text: 'When you unlock this door, put a +1/+1 counter on each of up to two target creatures.',
      },
    ],
    unlocked: [true, true],
    color: 'White',
    owner: 'viewer',
  },
  {
    oracle: 'room-walk-in-closet',
    halves: [
      {
        name: 'Walk-In Closet',
        cost: [{ Generic: 2 }, G],
        text: 'You may play lands from your graveyard.',
      },
      {
        name: 'Forgotten Cellar',
        cost: [{ Generic: 3 }, G, G],
        text: 'When you unlock this door, you may cast spells from your graveyard this turn, and if a card would be put into your graveyard from anywhere this turn, exile it instead.',
      },
    ],
    unlocked: [false, true],
    color: 'Green',
    owner: 'rival',
  },
]

const face = (half: Half): CardFaceView => ({
  name: half.name,
  mana_cost: costText(half.cost),
  type_line: 'Enchantment — Room',
  oracle_text: half.text,
  power: null,
  toughness: null,
  loyalty: null,
  defense: null,
  colors: [],
})

/** What the card endpoint answers for a Room of the scene: its two halves, front first. */
export function roomDetail(oracleId: string): Promise<CardDetail> {
  const spec = SPECS.find((room) => room.oracle === oracleId)
  if (spec === undefined) return Promise.reject(new Error(`no Room ${oracleId}`))
  return Promise.resolve({ faces: spec.halves.map(face) } as unknown as CardDetail)
}

/** The card client of the scene: the Rooms by their oracle ids, nothing else. */
export const roomsApi: CardsApi = {
  search: () => Promise.reject(new Error('The Rooms scene has no card search.')),
  get: (oracleId) => roomDetail(oracleId),
}

/** The name the server gives a Room with these doors unlocked (CR 709.5c): none, the one door, or the whole card. */
const nameOf = (spec: RoomSpec): string => {
  const [first, second] = spec.halves
  const [a, b] = spec.unlocked
  if (a && b) return `${first.name} // ${second.name}`
  return a ? first.name : b ? second.name : ''
}

/** The cost of the face in use: nothing while both doors are locked, the unlocked door's, or both added up. */
const costOf = (spec: RoomSpec): ManaCost | null => {
  const [first, second] = spec.halves
  const [a, b] = spec.unlocked
  if (a && b) {
    const generic = [first, second].reduce(
      (sum, half) =>
        sum +
        half.cost.reduce(
          (n, symbol) => n + (typeof symbol === 'object' && 'Generic' in symbol ? symbol.Generic : 0),
          0,
        ),
      0,
    )
    const colours = [...first.cost, ...second.cost].filter(
      (symbol) => typeof symbol === 'object' && 'Colored' in symbol,
    )
    return [{ Generic: generic }, ...colours]
  }
  return a ? first.cost : b ? second.cost : null
}

const viewOf = (id: ObjectId, spec: RoomSpec, viewer: PlayerId, rival: PlayerId): ObjectView => {
  const seat = spec.owner === 'viewer' ? viewer : rival
  return {
    id,
    owner: seat,
    controller: seat,
    zone: 'Battlefield',
    name: nameOf(spec),
    mana_cost: costOf(spec),
    supertypes: [],
    types: ['Enchantment'],
    subtypes: ['Room'],
    colors: [spec.color],
    power: null,
    toughness: null,
    abilities: [],
    permanent: { tapped: false, damage: 0, summoning_sick: false, doors: spec.unlocked },
    identity: { oracle_id: spec.oracle },
    ...(spec.unlocked[0] !== spec.unlocked[1] ? { face: spec.unlocked[0] ? 0 : 1 } : {}),
  }
}

/** The fixture with the four Rooms on the battlefield. */
export function roomsTable(fixture: Fixture): Fixture {
  const { view } = fixture
  const rival = view.players.find((player) => player.seat !== view.viewer)?.seat ?? view.viewer
  const base = Math.max(0, ...view.objects.map((object) => object.id)) + 1
  const rooms = SPECS.map((spec, at) => viewOf(base + at, spec, view.viewer, rival))
  const cards = [...(view.battlefield.cards ?? []), ...rooms.map((room) => room.id)]
  return {
    ...fixture,
    view: {
      ...view,
      objects: [...view.objects, ...rooms],
      battlefield: { count: cards.length, cards },
      // The viewer's own main phase with nothing on the stack: unlocking a door is a special action at sorcery speed.
      turn: { ...view.turn, active: view.viewer, step: 'Main1' },
      stack: { count: 0, cards: [] },
      priority: { holder: view.viewer, passed: [] },
    },
  }
}

/** The locked doors of the viewer's Rooms on the table, as the server offers them. */
function unlockable(fixture: Fixture): FaceOption[] {
  const { view } = fixture
  return SPECS.flatMap((spec) => {
    const object = view.objects.find((o) => o.identity?.oracle_id === spec.oracle)
    if (object === undefined || object.controller !== view.viewer) return []
    return spec.halves.flatMap((half, at): FaceOption[] =>
      spec.unlocked[at] === true
        ? []
        : [{ card: object.id, face: at, name: half.name, cost: half.cost, land: false }],
    )
  })
}

/** `base` with each locked door of the viewer's Rooms offered to unlock (`PriorityOptions.unlockable`). */
export function withRooms(
  base: (fixture: Fixture) => PendingDecision,
): (fixture: Fixture) => PendingDecision {
  return (fixture) => {
    const pending = base(fixture)
    const { kind } = pending.decision
    if (!('Priority' in kind)) return pending
    const options: PriorityOptions = { ...kind.Priority, unlockable: unlockable(fixture) }
    return { ...pending, decision: { ...pending.decision, kind: { Priority: options } } }
  }
}

/** The dev table's `?moments=1`: one batch holding a reveal of cards, a d20 roll and a coin flip, to look at the moments over the table; `shown=<n>` sets how many cards. */
import type { PlayState } from '@/play/store'
import type { CardsApi } from '@/net/cards'
import type { CardDetail, CardFaceView, EventBatch, ObjectView } from '@/protocol/generated'
import { tableOf } from './fxScenarios'

/** How many cards the reveal shows unless `shown` says otherwise. */
export const REVEALED_CARDS = 3

/** The result the die lands on. */
export const ROLL_RESULT = 17

/** A card of the reveal: the name it is shown under and the rules text the cards API answers for it. */
interface Shown {
  name: string
  text: string
}

/** What the revealed cards are called and say, in turn: some one line, some as long as the longest real cards. */
export const SHOWN: readonly Shown[] = [
  { name: 'Lightning Bolt', text: 'Lightning Bolt deals 3 damage to any target.' },
  {
    name: "Elspeth, Sun's Champion",
    text: '+1: Create three 1/1 white Soldier creature tokens.\n−3: Destroy all creatures with power 4 or greater.\n−7: You get an emblem with "Creatures you control get +2/+2 and have flying."',
  },
  {
    name: 'Sword of Fire and Ice',
    text: 'Equipped creature gets +2/+2 and has protection from red and from blue.\nWhenever equipped creature deals combat damage to a player, Sword of Fire and Ice deals 2 damage to any target and you draw a card.\nEquip {2}',
  },
  {
    name: 'Kozilek, Butcher of Truth, Whom Nobody Calls by Its Short Name',
    text: 'When you cast this spell, draw four cards.\nAnnihilator 4 (Whenever this creature attacks, defending player sacrifices four permanents.)\nWhen Kozilek, Butcher of Truth is put into a graveyard from anywhere, its owner shuffles their graveyard into their library.',
  },
  { name: 'Island', text: '({T}: Add {U}.)' },
  {
    name: 'The Ur-Dragon',
    text: 'Eminence — As long as The Ur-Dragon is in the command zone or on the battlefield, other Dragon spells you cast cost {1} less to cast.\nFlying\nWhenever one or more Dragons you control attack, draw that many cards, then you may put a permanent card from your hand onto the battlefield.',
  },
  {
    name: 'Mystic Remora',
    text: 'Cumulative upkeep {1} (At the beginning of your upkeep, put an age counter on this permanent, then sacrifice it unless you pay its upkeep cost for each age counter on it.)\nWhenever an opponent casts a noncreature spell, you may draw a card unless that player pays {4}.',
  },
  {
    name: 'Cyclonic Rift',
    text: 'Return target nonland permanent you don\'t control to its owner\'s hand.\nOverload {6}{U} (You may cast this spell for its overload cost. If you do, change "target" in its text to "each.")',
  },
  { name: 'Counterspell', text: 'Counter target spell.' },
  {
    name: 'Rhystic Study',
    text: 'Whenever an opponent casts a spell, you may draw a card unless that player pays {1}.',
  },
  {
    name: 'Craterhoof Behemoth',
    text: 'Haste\nWhen Craterhoof Behemoth enters, creatures you control gain trample and get +X/+X until end of turn, where X is the number of creatures you control.',
  },
  {
    name: 'Swords to Plowshares',
    text: 'Exile target creature. Its controller gains life equal to its power.',
  },
]

const face = (spec: Shown): CardFaceView => ({
  name: spec.name,
  mana_cost: '',
  type_line: 'Card',
  oracle_text: spec.text,
  power: null,
  toughness: null,
  loyalty: null,
  defense: null,
  colors: [],
})

/** The oracle id the card at `index` of the reveal carries, which `revealApi` answers for. */
export const shownOracleId = (index: number): string => `reveal-${index % SHOWN.length}`

/** The card client of the reveal scene: the rules text of `SHOWN` by oracle id, nothing else. */
export const revealApi: CardsApi = {
  search: () => Promise.reject(new Error('The reveal scene has no card search.')),
  get(oracleId) {
    const spec = SHOWN.find((_, index) => shownOracleId(index) === oracleId)
    if (spec === undefined) return Promise.reject(new Error(`no card ${oracleId}`))
    return Promise.resolve({ faces: [face(spec)] } as unknown as CardDetail)
  },
}

const inHand = (object: ObjectView, seat: number): boolean =>
  typeof object.zone === 'object' && 'Hand' in object.zone && object.zone.Hand === seat

/**
 * The batch of the first opponent revealing `count` cards of its hand (copies of the viewer's hand cards under new ids,
 * named and worded from `SHOWN`) and the viewer rolling a d20, then the opponent calling heads on a coin that comes up
 * tails. Null without a table, an opponent or a hand.
 */
export function momentsBatch(state: PlayState, count = REVEALED_CARDS): EventBatch | null {
  const { view, seq } = state
  if (view === null || seq === null) return null
  const rival = view.players.find((player) => player.seat !== view.viewer)?.seat
  const hand = view.objects.filter((object) => inHand(object, view.viewer))
  if (rival === undefined || hand.length === 0) return null
  const base = Math.max(...view.objects.map((object) => object.id)) + 1
  const shown = Array.from({ length: count }, (_, index): ObjectView => {
    const card = hand[index % hand.length] as ObjectView
    return {
      ...card,
      id: base + index,
      name: SHOWN[index % SHOWN.length]?.name ?? card.name,
      identity: { oracle_id: shownOracleId(index) },
      owner: rival,
      controller: rival,
      zone: { Hand: rival },
    }
  })
  const next = structuredClone(view)
  // A batch that changes nothing in the view is not told apart from a decision, so the turn counter moves.
  next.turn = { ...next.turn, number: next.turn.number + 1 }
  return {
    from_seq: seq,
    events: [
      { Public: { Revealed: { player: rival, cards: shown.map((card) => card.id) } } },
      { Public: { DieRolled: { player: view.viewer, sides: 20, result: ROLL_RESULT } } },
      { Public: { CoinFlipped: { player: rival, won: false, call: 'Heads', side: 'Tails' } } },
    ],
    table: tableOf(next),
    objects: [],
    gone: [],
    shown,
  }
}

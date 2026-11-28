/** The runs `/dev/delve` shows: one per state of a run, with a fuller party and ladder than the tests use. */
import { CONTENT, delveView, hero } from '../fakeDelves'
import type { DelveContent, DelveFightView, DelveView, Offer, Resolved } from '@/protocol/generated'

/** The states `/dev/delve?state=` can show. */
export const DEV_STATES = [
  'lobby',
  'map',
  'fight',
  'rework',
  'relics',
  'cards',
  'shop',
  'campfire',
  'event',
  'bargain',
  'discard',
  'surrender',
  'result',
  'victory',
  'defeat',
  'descent',
] as const

/** One state of `/dev/delve`. */
export type DevState = (typeof DEV_STATES)[number]

/** Reads `?state=`; the lobby when missing or unknown. */
export const devState = (value: unknown): DevState => DEV_STATES.find((state) => state === value) ?? 'lobby'

/** A boss's fight: its innate at level III, this fight's modifier, two phases by round, its Satchel and two
 * copies of a relic, worded as the design's Skrix. */
export const DEV_REWORKED_FIGHT: DelveFightView = {
  boss: 'Skrix, the Rat Queen',
  life: 90,
  innate: {
    name: 'Brood III',
    text: 'At the beginning of your upkeep, create three 1/1 black Rat creature tokens.',
    level: 3,
  },
  modifier: {
    name: 'Deep Pockets',
    text: "At the beginning of each player's draw step, that player draws an additional card.",
  },
  round_phases: [
    {
      name: 'The Nest Stirs',
      text: 'When this phase begins, create a Rat for each creature the heroes control. Rats you control have menace.',
      round: 8,
      begun: false,
    },
    {
      name: 'Plague Tide',
      text: 'When this phase begins, double the number of Rats you control. Creature tokens you control get +2/+0.',
      round: 14,
      begun: false,
    },
  ],
  relics: [
    { name: 'Whetstone', text: 'Creatures you control get +1/+0.' },
    { name: 'Whetstone', text: 'Creatures you control get +1/+0.' },
  ],
  satchel: [
    { name: 'Rat Out', reusable: true, tax: 0 },
    { name: 'Feed the Swarm', reusable: true, tax: 0 },
    { name: 'Village Rites', reusable: true, tax: 0 },
    { name: 'Second Harvest', reusable: false, tax: 0 },
  ],
  personality: {
    name: 'Spiteful',
    text: 'Hunts the healer: eyes the hero with the most on the table, each life they gained in the last rounds counting 2 more.',
  },
}

/** The content: the test theme with a third boss and more relics. */
export const DEV_CONTENT: DelveContent = {
  ...CONTENT,
  cards: [
    ...CONTENT.cards,
    {
      id: 'mend',
      name: 'Mending Lantern',
      kind: 'Relic',
      type_line: 'Relic',
      text: 'At the beginning of your upkeep, you gain 2 life.',
      art: 'mend',
      role: 'Healer',
      price: 60,
      rarity: 'Common',
    },
    {
      id: 'tithe',
      name: 'Tithe of Plenty',
      kind: 'Relic',
      type_line: 'Relic — Risky',
      text: 'Each player draws an additional card at the beginning of their draw step.',
      art: 'tithe',
      role: 'Risky',
      price: 0,
      rarity: 'Mythic',
    },
  ],
  bosses: [
    ...CONTENT.bosses.filter((b) => b.id !== 'grimgut'),
    {
      id: 'hollow',
      name: 'The Hollow Choir',
      colors: 'UB',
      line: 'Sing with us.',
      final_boss: false,
      innate: 'brood_i',
      round_phases: ['nest'],
      life: [40, 15],
      art: { card: 'Gisa, the Hellraiser', oracle_id: '696363c9-e8a7-4c65-b65c-4a94891ec43d' },
    },
    {
      id: 'grimgut',
      name: 'Grimgut',
      colors: 'BR',
      line: 'Kneel.',
      final_boss: true,
      innate: 'brood_i',
      round_phases: ['nest'],
      life: [40, 15],
      art: { card: 'Krenko, Tin Street Kingpin', oracle_id: 'e8065e1d-e937-4b56-8011-78f0d07328a0' },
    },
  ],
}

const ATRAXA = {
  name: 'Atraxa',
  creature: true,
  planeswalker: false,
  power: 4,
  generic: 3,
  keywords: ['Flying', 'Vigilance', 'Deathtouch', 'Lifelink'],
}

const party = [
  hero({
    name: 'Ana',
    deck: { id: 'd-atraxa', name: 'Atraxa Counters', colors: 'WUBG', commanders: [ATRAXA] },
    life: 31,
    gold: 60,
    relics: ['idol', 'mend'],
    satchel: ['Lightning Bolt', 'Cultivate'],
    upgrades: [
      { commander: 'Atraxa', boon: 'Counter' },
      { commander: 'Atraxa', boon: { Tax: 1 } },
    ],
    life_cap: -5,
    lost: ['Opt'],
  }),
  hero({
    name: 'Bo',
    user: 'u1',
    deck: { id: 'precon:cmm', name: 'Draconic Dissent', colors: 'UBR', commanders: [] },
    life: 18,
    gold: 25,
    relics: ['pact'],
  }),
  hero({
    name: 'Cyra',
    user: null,
    bot: 'Normal',
    deck: { id: 'precon:x', name: 'Elven Council', colors: 'G', commanders: [] },
    life: 40,
    gold: 50,
  }),
]

const map = {
  nodes: [
    { id: 0, row: 0, lane: 0, kind: 'boss' as const, boss: 'skrix_nest', next: [1, 2] },
    { id: 1, row: 1, lane: 0, kind: 'shop' as const, next: [3] },
    { id: 2, row: 1, lane: 1, kind: 'event' as const, next: [3] },
    { id: 3, row: 2, lane: 0, kind: 'boss' as const, boss: 'hollow', next: [4, 5] },
    { id: 4, row: 3, lane: 0, kind: 'treasure' as const, next: [6] },
    { id: 5, row: 3, lane: 1, kind: 'campfire' as const, next: [6] },
    { id: 6, row: 4, lane: 0, kind: 'boss' as const, boss: 'grimgut', next: [] },
  ],
}

/** The camp's answers: Ana's gamble won a relic she now picks, Bo paid with life, Cyra turned back. */
const RESULTS: Resolved[] = [
  {
    hero: 0,
    what: 'chose “Reach into the sack”',
    outcomes: ['Relic'],
    rolls: [{ odds: 50, won: true }],
  },
  { hero: 1, what: 'chose “Push past”', outcomes: [{ Life: -8 }, { Gold: 40 }], rolls: [] },
  { hero: 2, what: 'chose “Turn back”', outcomes: [], rolls: [] },
]

const OFFERS: Partial<Record<DevState, Offer>> = {
  relics: { Relics: { relics: ['idol', 'tithe', 'mend'] } },
  cards: { Cards: { cards: ['Lightning Bolt', 'Beast Within', 'Swords to Plowshares'] } },
  shop: {
    Shop: {
      items: [
        { Card: { name: 'Cultivate', sold: false } },
        { Card: { name: 'Counterspell', sold: true } },
        { Relic: { id: 'mend', price: 60, sold: false } },
        { Relic: { id: 'idol', price: 70, sold: false } },
        'Draught',
        { Upgrade: { upgrade: { commander: 'Atraxa', boon: { Keyword: 'Flying' } }, sold: false } },
      ],
    },
  },
  campfire: {
    Campfire: {
      upgrades: [
        { commander: 'Atraxa', boon: { Cheaper: 1 } },
        { commander: 'Atraxa', boon: { Pump: 2 } },
        { commander: 'Atraxa', boon: { Keyword: 'Trample' } },
      ],
    },
  },
  event: { Event: { event: 'toll' } },
  bargain: { Event: { event: 'bargain' } },
  discard: 'Discard',
  surrender: { Surrender: { relics: ['idol', 'mend'] } },
  result: { Relics: { relics: ['idol'] } },
  descent: { Relics: { relics: ['idol', 'tithe', 'mend'] } },
}

/** The run of one state. */
export function devView(state: DevState): DelveView {
  const log = ['Ana joined the party.', 'Bo joined the party.', 'Cyra (bot) joined the party.']
  if (state === 'lobby') {
    return delveView(
      { seed: 'EMBER-7Q2K' },
      {
        map,
        heroes: [party[0]!, { ...party[1]!, deck: null }, party[2]!],
        log,
      },
    )
  }
  const offer = OFFERS[state]
  // The event hero is short of gold, so the toll shows why it is shut.
  const heroes = party.map((h, i) =>
    i === 0 && offer ? { ...h, offers: [offer], ...(state === 'event' ? { gold: 18 } : {}) } : h,
  )
  const fighting = state === 'fight' || state === 'rework'
  const status =
    state === 'map'
      ? ('Map' as const)
      : fighting
        ? { Fight: { game: 'g-dev' } }
        : state === 'victory'
          ? ('Victory' as const)
          : state === 'defeat'
            ? ('Defeat' as const)
            : ('Room' as const)
  const fight =
    state === 'fight'
      ? { ...DEV_REWORKED_FIGHT, boss: 'The Hollow Choir', life: 140 }
      : state === 'rework'
        ? DEV_REWORKED_FIGHT
        : undefined
  return delveView(
    {
      seed: 'EMBER-7Q2K',
      boss: state === 'rework' ? 'skrix_nest' : 'hollow',
      mirror: ['Lightning Bolt', 'Cultivate', 'Sol Ring', 'Swords to Plowshares', 'Beast Within'],
      boss_lives: [
        ['skrix_nest', 40],
        ['hollow', 140],
        ['grimgut', 170],
      ],
      ...(fight === undefined ? {} : { fight }),
    },
    {
      map,
      heat: 1,
      hearts: state === 'defeat' ? 0 : 2,
      heroes,
      at: fighting ? 3 : state === 'result' ? 2 : state === 'descent' ? 6 : 0,
      cleared: fighting ? [0, 1] : state === 'victory' || state === 'descent' ? [0, 1, 3, 5, 6] : [0],
      descending: state === 'descent',
      floors_cleared: state === 'descent' ? 1 : 0,
      votes: state === 'map' ? { 1: 1 } : {},
      status,
      log: [...log, 'The party descended.', 'Skrix, the Rat Queen fell.'],
      results: state === 'result' ? RESULTS : [],
    },
  )
}

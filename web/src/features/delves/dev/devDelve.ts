/** The runs `/dev/delve` shows: one per state of a run, with a fuller party and ladder than the tests use. */
import { CONTENT, delveView, hero } from '../fakeDelves'
import type { DelveContent, DelveView, Offer } from '@/protocol/generated'

/** The states `/dev/delve?state=` can show. */
export const DEV_STATES = [
  'lobby',
  'map',
  'fight',
  'relics',
  'cards',
  'shop',
  'campfire',
  'event',
  'victory',
  'defeat',
] as const

/** One state of `/dev/delve`. */
export type DevState = (typeof DEV_STATES)[number]

/** Reads `?state=`; the lobby when missing or unknown. */
export const devState = (value: unknown): DevState => DEV_STATES.find((state) => state === value) ?? 'lobby'

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
    },
    {
      id: 'grim',
      name: 'Grimgut, Tunnel King',
      kind: 'Boss',
      type_line: 'Boss — Ogre',
      text: 'Whenever Grimgut attacks, each hero sacrifices a creature.',
      art: 'grim',
      role: '',
      price: 0,
    },
  ],
  bosses: [
    ...CONTENT.bosses.filter((b) => b.id !== 'grimgut'),
    {
      id: 'hollow',
      name: 'The Hollow Choir',
      colors: 'UB',
      line: 'Sing with us.',
      card: null,
      phases: [],
      life: [32, 36],
    },
    {
      id: 'grimgut',
      name: 'Grimgut',
      colors: 'BR',
      line: 'Kneel.',
      card: 'grim',
      phases: [['nest', 50]],
      life: [36, 40],
    },
  ],
}

const party = [
  hero({
    name: 'Ana',
    deck: { id: 'd-atraxa', name: 'Atraxa Counters', colors: 'WUBG' },
    life: 31,
    gold: 60,
    relics: ['idol', 'mend'],
  }),
  hero({
    name: 'Bo',
    user: 'u1',
    deck: { id: 'precon:cmm', name: 'Draconic Dissent', colors: 'UBR' },
    life: 18,
    gold: 25,
    relics: ['pact'],
  }),
  hero({
    name: 'Cyra',
    user: null,
    bot: 'Normal',
    deck: { id: 'precon:x', name: 'Elven Council', colors: 'G' },
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
      ],
    },
  },
  campfire: 'Campfire',
  event: { Event: { event: 'toll' } },
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
  const heroes = party.map((h, i) => (i === 0 && offer ? { ...h, offers: [offer] } : h))
  const status =
    state === 'map'
      ? ('Map' as const)
      : state === 'fight'
        ? { Fight: { game: 'g-dev' } }
        : state === 'victory'
          ? ('Victory' as const)
          : state === 'defeat'
            ? ('Defeat' as const)
            : ('Room' as const)
  const fight =
    state === 'fight'
      ? {
          boss: 'The Hollow Choir',
          life: 140,
          rules: ['grim', 'skrix', 'skrix'],
          phases: [['nest', 93]] as Array<[string, number]>,
          minions: [['Brood of Skrix', 25]] as Array<[string, number]>,
        }
      : undefined
  return delveView(
    { seed: 'EMBER-7Q2K', boss: 'hollow', ...(fight === undefined ? {} : { fight }) },
    {
      map,
      heat: 1,
      hearts: state === 'defeat' ? 0 : 2,
      heroes,
      at: state === 'fight' ? 3 : 0,
      cleared: state === 'fight' ? [0, 1] : state === 'victory' ? [0, 1, 3, 5, 6] : [0],
      votes: state === 'map' ? { 1: 1 } : {},
      status,
      log: [...log, 'The party descended.', 'Skrix, the Rat Queen fell.'],
    },
  )
}

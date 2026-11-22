/**
 * Made-up numbers for `/dev/support`: a month of daily snapshots rising to about 94.7%, a feed of each kind and about
 * 1,700 missing cards of every state, built the same way on every load.
 */
import type { CardSupport, FeedEvent, MissingCards, MissingState, SupportPoint } from '@/protocol/generated'

/** Marks the fixture in the page, so a production build can be checked for it. */
export const SUPPORT_FIXTURE_MARKER = 'dev-support-fixture'

const CARDS = 32_116
const DAYS = 30
const START = Date.parse('2026-10-24T03:00:00Z')
const DAY = 86_400_000

const at = (day: number, hour = 3): string =>
  new Date(START + day * DAY + (hour - 3) * 3_600_000).toISOString().replace('.000', '')

/** The trend: one snapshot a day, the share rising unevenly. */
export const FIXTURE_HISTORY: SupportPoint[] = Array.from({ length: DAYS }, (_, day) => ({
  taken_at: at(day),
  release: `v0.2.${100 + Math.floor(day / 2)}`,
  cards: CARDS - (DAYS - 1 - day) * 8,
  automatic: 28_900 + Math.round(day * 75 + 40 * Math.sin(day)),
}))

const last = FIXTURE_HISTORY[FIXTURE_HISTORY.length - 1] ?? {
  taken_at: at(0),
  release: null,
  cards: CARDS,
  automatic: 0,
}

const FEED: FeedEvent[] = [
  {
    kind: 'now_automatic',
    at: at(29, 4),
    release: 'v0.2.131',
    new_release: true,
    count: 170,
    names: [
      'Sol Ring',
      'Rhystic Study',
      'Smothering Tithe',
      'Cyclonic Rift',
      'Dockside Extortionist',
      'Fierce Guardianship',
    ],
  },
  {
    kind: 'new_precons',
    at: at(29),
    code: 'STC',
    set_name: 'Star Trek Commander',
    decks: ['Final Frontier', 'Borg Collective', 'Klingon Honor', 'Federation Diplomacy'],
  },
  {
    kind: 'new_cards',
    at: at(29),
    count: 289,
    names: [
      'James T. Kirk, Captain',
      'Spock, Science Officer',
      'Jean-Luc Picard',
      'Beam Me Up',
      'Locutus of Borg',
      'Data, Android Officer',
    ],
  },
  {
    kind: 'check_failed',
    at: at(26),
    failures: ['fetch: Could not reach mtgjson.com (timeout after 3 tries)'],
  },
  { kind: 'now_automatic', at: at(24, 5), release: 'v0.2.128', new_release: true, count: 0, names: [] },
  {
    kind: 'now_automatic',
    at: at(20, 4),
    release: 'v0.2.125',
    new_release: false,
    count: 12,
    names: ['Kodama of the West Tree', 'Esper Sentinel'],
  },
]

const STATES: MissingState[] = ['partial', 'partial', 'manual', 'waiting', 'partial', 'manual', 'other']
const BLOCKERS = [
  'A triggered ability the game cannot read yet',
  'An activated ability the game cannot read yet',
  'The engine does not run “vote” yet',
  'A static ability the game cannot read yet',
  'You cast or turn over the back face by hand: the game plays only the front face of a card',
  'No rules yet',
  'Its Oracle text changed since its rules were written',
]
const WORDS = ['Ancient', 'Chaos', 'Mirror', 'Ember', 'Tide', 'Grave', 'Storm', 'Thorn', 'Æther', 'Night']
const KINDS = [
  'Wurm',
  'Oracle',
  'Familiar',
  'Gateway',
  'Pact',
  'Sphinx',
  'Ritual',
  'Golem',
  'Sentinel',
  'Lich',
]

/** About 1,700 missing cards, the most played first, the last hundred without a rank. */
export const FIXTURE_MISSING: MissingCards = {
  snapshot_id: DAYS,
  cards: Array.from({ length: CARDS - last.automatic }, (_, index) => {
    const state = index % 37 === 3 ? 'waiting' : (STATES[index % STATES.length] ?? 'partial')
    const blocker =
      state === 'waiting'
        ? 'Waits for a player to read its rules'
        : (BLOCKERS[(index * 3) % BLOCKERS.length] ?? '')
    return {
      oracle_id: `00000000-0000-4000-8000-${String(index).padStart(12, '0')}`,
      name: `${WORDS[index % WORDS.length] ?? ''} ${KINDS[Math.floor(index / 10) % KINDS.length] ?? ''} ${Math.floor(index / 100) + 1}`,
      state,
      blocker,
      detail: state === 'partial' ? 'triggered: When ~ enters, each opponent votes for N' : '',
      blockers: 1 + (index % 3),
      rank: index > CARDS - last.automatic - 100 ? null : 40 + index * 9,
    }
  }),
}

/** The overview of the fixture. */
export const FIXTURE_OVERVIEW: CardSupport = {
  snapshot: {
    id: DAYS,
    taken_at: last.taken_at,
    release: last.release,
    cards: last.cards,
    automatic: last.automatic,
    states: (() => {
      const counts = { waiting: 0, partial: 0, manual: 0, other: 0 }
      for (const card of FIXTURE_MISSING.cards) counts[card.state] += 1
      const read = 4_863
      const unverified = 2_935
      return { verified: last.automatic - read - unverified, read, unverified, ...counts }
    })(),
    buckets: [
      { label: 'top 100', cards: 100, verified: 92, automatic: 99 },
      { label: 'top 500', cards: 500, verified: 431, automatic: 489 },
      { label: 'top 1000', cards: 1_000, verified: 846, automatic: 972 },
      { label: 'top 2000', cards: 2_000, verified: 1_640, automatic: 1_921 },
      { label: 'top 5000', cards: 5_000, verified: 3_980, automatic: 4_760 },
      { label: 'all ranked', cards: 31_000, verified: 21_900, automatic: 29_360 },
      { label: 'no rank', cards: 1_116, verified: 722, automatic: 960 },
    ],
    missing: FIXTURE_MISSING.cards.length,
  },
  history: FIXTURE_HISTORY,
  last_check: {
    started_at: at(29),
    finished_at: at(29, 4),
    state: 'succeeded',
    scryfall_updated_at: '2026-11-22T02:04:11.120+00:00',
    mtgjson_build: '5.3.0+20261122',
  },
  feed: FEED,
}

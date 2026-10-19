/** The deck behind `/dev/builder`: a card of each support state and illegal lines, as a server would answer it. */
import {
  cardHit,
  deckView,
  legalityIssue,
  oracleIdOf,
  reportCard,
  reportOf,
} from '@/features/decks/fakeDecks'
import type {
  CardHit,
  DeckLineView,
  DeckState,
  DeckView,
  DeckZone,
  Reason,
  ReportCard,
  Support,
} from '@/protocol/generated'

/** The text the dev page prints, so a production bundle can be searched for any trace of the fixture. */
export const BUILDER_FIXTURE_MARKER = 'dev-builder-fixture-3c9e'

/** The decks `/dev/builder?deck=` can show. */
export const DEV_DECKS = ['mixed', 'manual', 'verified', 'changed', 'unchecked'] as const

/** One of the decks `/dev/builder` shows. */
export type DevDeck = (typeof DEV_DECKS)[number]

interface Entry {
  name: string
  zone?: DeckZone
  count: number
  support: Support
  reasons?: Reason[]
  type?: string
  cost?: string
}

const reason = (text: string, face = 0): Reason => ({ face, span: { start: 0, end: 60 }, text })

// The entries by support state: the first of each state is the one the others of the fixture are built around.
const CLEAN: Entry[] = [
  {
    name: 'Krenko, Mob Boss',
    zone: 'commander',
    count: 1,
    support: 'verified',
    type: 'Legendary Creature',
    cost: '{2}{R}{R}',
  },
  { name: 'Goblin Guide', count: 1, support: 'verified', type: 'Creature — Goblin Scout', cost: '{R}' },
  { name: 'Duress', zone: 'sideboard', count: 2, support: 'verified', type: 'Sorcery', cost: '{B}' },
]
const BY_HAND: Entry[] = [
  {
    name: 'Goblin Rabblemaster',
    count: 1,
    support: 'partial',
    type: 'Creature — Goblin Warrior',
    cost: '{2}{R}',
    reasons: [reason('the engine does not run attack requirements yet')],
  },
  {
    name: 'Bonecrusher Giant // Stomp',
    count: 1,
    support: 'partial',
    type: 'Creature — Giant',
    cost: '{2}{R}',
    reasons: [reason('the engine does not run damage prevention yet', 1)],
  },
  {
    name: 'Chaos Orb',
    count: 1,
    support: 'manual',
    type: 'Artifact',
    cost: '{2}',
    reasons: [reason('no rules for this card yet; the table resolves the whole text')],
  },
  {
    name: 'Goblin Welder',
    count: 1,
    support: 'stale',
    type: 'Creature — Goblin Artificer',
    cost: '{R}',
    reasons: [reason('the oracle text changed since the rules were written')],
  },
  {
    name: 'Pithing Needle',
    zone: 'sideboard',
    count: 1,
    support: 'manual',
    type: 'Artifact',
    cost: '{1}',
    reasons: [reason('no rules for this card yet; the table resolves the whole text')],
  },
]
const BLOCKING: Entry[] = [
  {
    name: 'Cogwork Librarian',
    count: 1,
    support: 'unplayable',
    type: 'Artifact Creature — Construct',
    cost: '{3}',
    reasons: [reason('the card changes how a draft works, which this game does not run')],
  },
]
const ILLEGAL: Entry[] = [
  { name: 'Lightning Bolt', count: 4, support: 'verified', type: 'Instant', cost: '{R}' },
  { name: 'Black Lotus', count: 1, support: 'verified', type: 'Artifact', cost: '{0}' },
]

// The longest card name there is (141 characters), which a search row has to fit at any width.
const LONGEST =
  'Our Market Research Shows That Players Like Really Long Card Names So We Made this Card to Have the Absolute Longest Card Name Ever Elemental'

/** The cards of the fixture as a search shows them, for the card server behind the page. */
export function fixtureCards(): CardHit[] {
  const searchOnly: Entry[] = [
    { name: 'Mountain', count: 0, support: 'verified' },
    { name: LONGEST, count: 0, support: 'manual', type: 'Creature — Elemental', cost: '{1}{G}' },
    {
      name: 'Delver of Secrets // Insectile Aberration',
      count: 0,
      support: 'verified',
      type: 'Creature — Human Wizard',
      cost: '{U}',
    },
  ]
  return [...CLEAN, ...BY_HAND, ...BLOCKING, ...ILLEGAL, ...searchOnly].map((entry) =>
    cardHit(entry.name, {
      support: entry.support,
      type_line: entry.type ?? 'Basic Land — Mountain',
      mana_cost: entry.cost ?? '',
    }),
  )
}

/**
 * A Commander deck for `/dev/builder`. `mixed` holds a card of each support state, a card the game cannot host and
 * illegal lines (4 copies of a singleton card, a banned card, the wrong size); `manual` is legal with cards played by
 * hand; `verified` has only verified cards; `changed` and `unchecked` are the mixed cards with no valid check.
 */
export function fixtureView(deck: DevDeck): DeckView {
  const entries =
    deck === 'verified'
      ? CLEAN
      : deck === 'manual'
        ? [...CLEAN, ...BY_HAND]
        : [...CLEAN, ...BY_HAND, ...BLOCKING, ...ILLEGAL]
  const legal = deck === 'verified' || deck === 'manual'
  const lands = legal
    ? 99 - entries.filter((e) => (e.zone ?? 'main') === 'main').reduce((n, e) => n + e.count, 0)
    : 30
  const all: Entry[] = [...entries, { name: 'Mountain', count: lands, support: 'verified' }]
  const lines: DeckLineView[] = all.map((entry) => ({
    zone: entry.zone ?? 'main',
    count: entry.count,
    printing_id: null,
    card: cardHit(entry.name, {
      support: entry.support,
      type_line: entry.type ?? 'Basic Land — Mountain',
      mana_cost: entry.cost ?? '',
    }),
  }))
  const cards: ReportCard[] = all.map((entry) =>
    reportCard(entry.name, {
      zone: entry.zone ?? 'main',
      count: entry.count,
      support: entry.support,
      reasons: entry.reasons ?? [],
    }),
  )
  const size = all.filter((e) => e.zone !== 'sideboard').reduce((n, e) => n + e.count, 0)
  const legality = legal
    ? []
    : [
        legalityIssue(`a Commander deck has exactly 100 cards with its commanders, this one has ${size}`, {
          code: 'deck_size',
        }),
        legalityIssue('Lightning Bolt: 4 copies, but only 1 allowed', {
          code: 'copies',
          zone: 'main',
          oracle_id: oracleIdOf('Lightning Bolt'),
          name: 'Lightning Bolt',
        }),
        legalityIssue('Black Lotus is banned in commander', {
          code: 'banned',
          zone: 'main',
          oracle_id: oracleIdOf('Black Lotus'),
          name: 'Black Lotus',
        }),
      ]
  const report = reportOf(cards, legality)
  const stateOf: Record<DevDeck, DeckState> = {
    mixed: report.summary.state,
    manual: report.summary.state,
    verified: report.summary.state,
    changed: 'unchecked',
    unchecked: 'unchecked',
  }
  const state = stateOf[deck]
  return deckView({
    id: 'dev-deck',
    name: 'Krenko Goblins',
    format: 'commander',
    cards: lines,
    state,
    verified: state === 'verified',
    changed: deck === 'changed',
    report,
  })
}

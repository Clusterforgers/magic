/**
 * Decks, reports and card texts for the not-validated list, in memory: four decks that share cards (one runs whole),
 * a card of every state, a clause on a back face and a card with no clause, for the tests and `/dev/unvalidated`.
 */
import type { CardsApi } from '@/net/cards'
import { DeckError } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { CardDetail, DeckListItem, DeckReport, Reason, ReportCard, Support } from '@/protocol/generated'
import { cardHit, deckIn, deckView, oracleIdOf, reportCard, reportOf, summary } from '../fakeDecks'

/** Text the production build must not contain (`devBuild.test.ts` looks for it). */
export const UNVALIDATED_FIXTURE_MARKER = 'unvalidated-fixture'

/** One clause: the words of the face it covers (the whole face when none) and why the game leaves it. */
export interface ClauseSpec {
  face?: number
  phrase?: string
  reason: string
}

/** A card of the fixture: its faces' texts, its state and its clauses. */
export interface CardSpec {
  name: string
  faces: readonly string[]
  support: Exclude<Support, 'verified'>
  clauses: readonly ClauseSpec[]
}

/** Where `phrase` lies in `text`, in Unicode scalar values like the server counts. */
export function spanOf(text: string, phrase: string): { start: number; end: number } {
  const at = text.indexOf(phrase)
  if (at < 0) throw new Error(`"${phrase}" is not in "${text}"`)
  return {
    start: Array.from(text.slice(0, at)).length,
    end: Array.from(text.slice(0, at + phrase.length)).length,
  }
}

export const CARDS: Readonly<Record<string, CardSpec>> = {
  rhystic: {
    name: 'Rhystic Study',
    faces: ['Whenever an opponent casts a spell, you may draw a card unless that player pays {1}.'],
    support: 'manual',
    clauses: [],
  },
  drain: {
    name: 'Mana Drain',
    faces: [
      'Counter target spell. At the beginning of your next main phase, add an amount of {C} equal to that spell’s mana value.',
    ],
    support: 'partial',
    clauses: [
      {
        phrase:
          'At the beginning of your next main phase, add an amount of {C} equal to that spell’s mana value.',
        reason: 'the delayed mana is not implemented',
      },
    ],
  },
  season: {
    name: 'Doubling Season',
    faces: [
      'If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead.\nIf an effect would put one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead.',
    ],
    support: 'partial',
    clauses: [
      {
        phrase:
          'If an effect would put one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead.',
        reason: 'replacement effects for counters are not implemented',
      },
    ],
  },
  delver: {
    name: 'Delver of Secrets // Insectile Aberration',
    faces: [
      'At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If an Instant or Sorcery card is revealed this way, transform Delver of Secrets.',
      'Flying',
    ],
    support: 'partial',
    clauses: [
      { phrase: 'transform Delver of Secrets', reason: 'transforming is not implemented' },
      { face: 1, phrase: 'Flying', reason: 'flying of a back face is not implemented' },
    ],
  },
  remora: {
    name: 'Mystic Remora',
    faces: [
      'Cumulative upkeep {1}\nWhenever an opponent casts a noncreature spell, you may draw a card unless that player pays {4}.',
    ],
    support: 'stale',
    clauses: [],
  },
  contract: {
    name: 'Contract from Below',
    faces: [
      'Remove Contract from Below from your deck before playing if you’re not playing for ante.\nDiscard your hand, ante the top card of your library, then draw seven cards.',
    ],
    support: 'unplayable',
    clauses: [{ phrase: 'ante the top card of your library', reason: 'ante is not played in this game' }],
  },
}

const reasonsOf = (spec: CardSpec): Reason[] =>
  spec.clauses.map((clause) => {
    const face = clause.face ?? 0
    const text = spec.faces[face] ?? ''
    return {
      face,
      span: clause.phrase === undefined ? spanOf(text, text) : spanOf(text, clause.phrase),
      text: clause.reason,
    }
  })

/** The line of a report for a card of the fixture. */
export const reportLine = (key: keyof typeof CARDS, zone: ReportCard['zone'] = 'main'): ReportCard => {
  const spec = CARDS[key] as CardSpec
  return reportCard(spec.name, { zone, support: spec.support, reasons: reasonsOf(spec) })
}

/** What `GET /api/cards/{oracle_id}` answers for a card of the fixture. */
export function detailOf(key: keyof typeof CARDS): CardDetail {
  const spec = CARDS[key] as CardSpec
  return {
    card: cardHit(spec.name, { support: spec.support }),
    oracle_text: spec.faces.join('\n'),
    keywords: [],
    faces: spec.faces.map((text, index) => ({
      name: spec.name.split(' // ')[index] ?? spec.name,
      mana_cost: '',
      type_line: 'Card',
      oracle_text: text,
      power: null,
      toughness: null,
      loyalty: null,
      defense: null,
      colors: [],
    })),
    legalities: {},
    printings: [],
    rulings: [],
    reasons: reasonsOf(spec),
  }
}

/** A saved deck of the fixture: its list entry and its report. */
export interface FixtureDeck {
  item: DeckListItem
  report: DeckReport
}

function deckOf(
  id: string,
  name: string,
  cards: ReadonlyArray<ReportCard>,
  state: 'verified' | 'manual_play' = 'manual_play',
): FixtureDeck {
  const report = reportOf(cards)
  return {
    item: deckIn(state, {
      id,
      name,
      summary: summary({ state, counts: report.summary.counts, zones: report.summary.zones }),
    }),
    report,
  }
}

const filler = (count: number): ReportCard => reportCard('Forest', { count })

/** The decks of the fixture, most recently saved first. */
export const FIXTURE_DECKS: readonly FixtureDeck[] = [
  deckOf('deck-atraxa', 'Atraxa Superfriends', [
    reportLine('drain'),
    reportLine('season'),
    filler(95),
    reportLine('rhystic'),
  ]),
  deckOf('deck-burn', 'Mono Red Burn', [reportCard('Lightning Bolt', { count: 4 }), filler(56)], 'verified'),
  deckOf('deck-izzet', 'Izzet Spells', [
    reportLine('delver'),
    reportLine('rhystic'),
    reportLine('drain'),
    reportLine('drain', 'sideboard'),
    filler(80),
  ]),
  deckOf('deck-chaos', 'Chaos Brew With A Rather Long Name To Wrap', [
    reportLine('contract'),
    reportLine('remora'),
    reportLine('rhystic'),
    filler(90),
  ]),
]

/** How the fixture's servers misbehave; none by default. */
export interface FixtureOptions {
  /** The list of decks is refused with the code of a `DeckError`. */
  listFails?: 'decks_unavailable' | 'unauthenticated' | 'network'
  /** The ids of decks whose check fails. */
  checkFails?: readonly string[]
  /** The decks to hold; the fixture's by default. */
  decks?: readonly FixtureDeck[]
  /** Milliseconds each answer waits. */
  delayMs?: number
  /** The card texts are never read. */
  cardsFail?: boolean
}

const later = <T>(value: () => T, delayMs: number): Promise<T> =>
  new Promise((resolve, reject) => {
    setTimeout(() => {
      try {
        resolve(value())
      } catch (error) {
        reject(error instanceof Error ? error : new Error('failed'))
      }
    }, delayMs)
  })

/** The deck calls of a server holding the fixture's decks. */
export function fixtureDecksApi(options: FixtureOptions = {}): Pick<DecksApi, 'list' | 'get' | 'check'> & {
  asked: string[]
} {
  const decks = options.decks ?? FIXTURE_DECKS
  const delay = options.delayMs ?? 0
  const asked: string[] = []
  const find = (id: string): FixtureDeck => {
    const found = decks.find((deck) => deck.item.id === id)
    if (found === undefined) throw new DeckError('not_found', 404, 'no such deck')
    return found
  }
  return {
    asked,
    list: () =>
      later(() => {
        asked.push('list')
        if (options.listFails !== undefined) throw new DeckError(options.listFails, 503, 'refused')
        return decks.map((deck) => deck.item)
      }, delay),
    get: (id) =>
      later(() => {
        asked.push(`get ${id}`)
        const deck = find(id)
        return deckView({
          id: deck.item.id,
          name: deck.item.name,
          state: deck.item.state,
          verified: deck.item.verified,
          cards: [],
          report: deck.report,
        })
      }, delay),
    check: (request) =>
      later(() => {
        const id = request.deck ?? ''
        asked.push(`check ${id}`)
        if (options.checkFails?.includes(id) === true) throw new DeckError('internal', 500, 'failed')
        return find(id).report
      }, delay),
  }
}

/** The card endpoint of a server that knows the fixture's cards. */
export function fixtureCardsApi(options: FixtureOptions = {}): Pick<CardsApi, 'get'> & { asked: string[] } {
  const asked: string[] = []
  return {
    asked,
    get: (oracleId) =>
      later(() => {
        asked.push(oracleId)
        const key = Object.keys(CARDS).find((name) => oracleIdOf(CARDS[name]?.name ?? '') === oracleId)
        if (options.cardsFail === true || key === undefined) throw new Error('no such card')
        return detailOf(key)
      }, options.delayMs ?? 0),
  }
}

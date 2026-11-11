/** The cards to check of `/dev/read`: one saved deck with a partly and a wholly by-hand card, their texts, in memory. */
import {
  cardHit,
  deckIn,
  deckView,
  oracleIdOf,
  reportCard,
  reportOf,
  summary,
} from '@/features/decks/fakeDecks'
import type { ByHandApi } from '@/features/decks/byHand'
import type { CardsApi } from '@/net/cards'
import { DeckError } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { CardDetail, Reason, ReportCard, Support } from '@/protocol/generated'

interface Spec {
  name: string
  text: string
  support: Support
  /** The phrase of the text each reason covers, and the reason. */
  clauses: ReadonlyArray<[string, string]>
}

const SPECS: readonly Spec[] = [
  {
    name: 'Mana Drain',
    text: 'Counter target spell. At the beginning of your next main phase, add an amount of {C} equal to that spell’s mana value.',
    support: 'partial',
    clauses: [
      [
        'At the beginning of your next main phase, add an amount of {C} equal to that spell’s mana value.',
        'The delayed mana is not implemented',
      ],
    ],
  },
  {
    name: 'Rhystic Study',
    text: 'Whenever an opponent casts a spell, you may draw a card unless that player pays {1}.',
    support: 'manual',
    clauses: [],
  },
]

function reasonsOf(spec: Spec): Reason[] {
  return spec.clauses.map(([phrase, text]) => {
    const start = Array.from(spec.text.slice(0, Math.max(0, spec.text.indexOf(phrase)))).length
    return { face: 0, span: { start, end: start + Array.from(phrase).length }, text }
  })
}

const lines: ReportCard[] = [
  ...SPECS.map((spec) => reportCard(spec.name, { support: spec.support, reasons: reasonsOf(spec) })),
  reportCard('Island', { count: 60 }),
]

function detailOf(spec: Spec): CardDetail {
  return {
    card: cardHit(spec.name, { support: spec.support }),
    oracle_text: spec.text,
    keywords: [],
    faces: [],
    legalities: {},
    printings: [],
    rulings: [],
    reasons: reasonsOf(spec),
  }
}

/** The deck calls of a server holding the one deck `deck-izzet`, whose saves change nothing. */
export function devCheckDecks(empty: boolean): Pick<DecksApi, 'list' | 'get' | 'check'> & ByHandApi {
  const report = reportOf(empty ? [reportCard('Island', { count: 60 })] : lines)
  const view = deckView({ id: 'deck-izzet', name: 'Izzet Spells', report })
  const get = async (id: string) => {
    if (id !== view.id) throw new DeckError('not_found', 404, 'no such deck')
    return view
  }
  return {
    list: async () => [
      deckIn(empty ? 'verified' : 'manual_play', {
        id: view.id,
        name: view.name,
        summary: summary({ counts: report.summary.counts }),
      }),
    ],
    get,
    check: async () => report,
    update: (id) => get(id),
  }
}

/** The card endpoint for the cards of that deck. */
export function devCheckCards(base: CardsApi): CardsApi {
  return {
    ...base,
    get: async (oracleId) => {
      const spec = SPECS.find((entry) => oracleIdOf(entry.name) === oracleId)
      if (spec === undefined) throw new Error('no such card')
      return detailOf(spec)
    },
  }
}

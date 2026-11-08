/** Made-up bulk reads in memory, for the dev scene and the tests: the shapes `GET` and `PUT /api/reads/bulk` answer. */
import type { BulkReadsApi } from '@/net/bulkReads'
import type { BulkCard, BulkReads, BulkRule } from '@/protocol/generated'

/** Text only the fixture holds, so a check can find it in a build. */
export const BULK_READS_FIXTURE_MARKER = 'dev-bulk-read-fixture'

/** The rules the fixture's cards need, in the server's order. */
export const BULK_RULES: BulkRule[] = [
  {
    name: 'self_name',
    title: "The card's own name",
    summary: '"this creature" (or artifact, land, ...) and ~ both name the card.',
  },
  {
    name: 'duration_order',
    title: 'Duration first',
    summary: '"until end of turn" stands at the front instead of the end.',
  },
  {
    name: 'discard_hand',
    title: 'Discard your hand',
    summary: 'Discarding as many cards as are in your hand is discarding your hand.',
  },
]

const card = (name: string, rule: string, clauses: [string, string][]): BulkCard => ({
  oracle_id: `oracle-${name.toLowerCase().replaceAll(/[^a-z]+/g, '-')}`,
  name,
  read_hash: `hash-${name.length}`,
  rule,
  clauses: clauses.map(([printed, engine]) => ({ printed, engine, rules: [rule] })),
})

/** The fixture's cards, by name. */
export const BULK_CARDS: BulkCard[] = [
  card('Ancestral Statue', 'self_name', [
    [
      "When this creature enters, return a nonland permanent you control to its owner's hand.",
      "When ~ enters, return a nonland permanent you control to its owner's hand.",
    ],
  ]),
  card('Brute Force', 'duration_order', [
    ['Target creature gets +3/+3 until end of turn.', 'Until end of turn, target creature gets +3/+3.'],
  ]),
  card('Darksteel Myr', 'self_name', [
    ['This creature attacks each combat if able.', '~ attacks each combat if able.'],
  ]),
  card('Slate of Ancestry', 'discard_hand', [
    [
      '{4}, {T}, Discard your hand: Draw a card for each creature you control.',
      '{4}, {T}, Discard X cards, where X is the number of cards in your hand: Draw a card for each creature you control.',
    ],
  ]),
]

/** Options of the fixture client. */
export interface BulkFixtureOptions {
  /** No cards wait. */
  empty?: boolean
  /** Every approval meets a changed draft. */
  stale?: boolean
}

/** A client over the fixture: approved cards leave the list. */
export function fixtureBulkReadsApi(options: BulkFixtureOptions = {}): BulkReadsApi {
  let cards = options.empty === true ? [] : [...BULK_CARDS]
  const list = async (): Promise<BulkReads> => ({
    rules: BULK_RULES.filter((rule) => cards.some((c) => c.rule === rule.name)),
    cards,
  })
  return {
    list,
    approve: async ({ cards: picked }) => {
      const ids = picked.map((c) => c.oracle_id)
      if (options.stale === true) return { approved: [], stale: ids }
      cards = cards.filter((c) => !ids.includes(c.oracle_id))
      return { approved: ids, stale: [] }
    },
  }
}

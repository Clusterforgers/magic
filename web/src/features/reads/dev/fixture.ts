/** Made-up cards to read and an in-memory read server for `/dev/read` and the tests of the review screen. */
import { BY_HAND } from '@/lib/byHand'
import { DeckError } from '@/net/decks'
import type { ReadsApi } from '@/net/reads'
import type { ReadCard, ReadQueue, ReadRow } from '@/protocol/generated'

/** Text of the dev page that must never reach a production build. */
export const READS_FIXTURE_MARKER = 'dev-read-fixture'

function row(face: number, start: number, printed: string, rest: Partial<ReadRow>): ReadRow {
  return {
    face,
    span: { start, end: start + Array.from(printed).length },
    kind: 'Static',
    printed,
    state: 'written',
    abilities: [0],
    continues: false,
    label: 'static ability',
    wording: printed,
    steps: [{ depth: 0, text: printed.replace(/\.$/, '').toLowerCase() }],
    ...rest,
  }
}

function card(
  oracleId: string,
  name: string,
  rows: ReadRow[],
  rest: Partial<ReadCard> = {},
  faces: string[] = [name],
): ReadCard {
  return {
    oracle_id: oracleId,
    name,
    read_hash: `${oracleId.slice(0, 8)}0000000000000000000000aa`,
    state: 'pending',
    correct: 0,
    wrong: 0,
    forge: false,
    sheet: { card: name, faces, rows, extra: [] },
    mine: null,
    ...rest,
  }
}

/** The cards of the fixture by oracle id. */
export const READ_CARDS: Readonly<Record<string, ReadCard>> = {
  'f0000000-0000-4000-8000-000000000001': card(
    'f0000000-0000-4000-8000-000000000001',
    'Tidecaller of the Long Shoal',
    [
      row(0, 0, 'Flying', { kind: 'Keyword', label: 'keyword', steps: [{ depth: 0, text: 'has flying' }] }),
      row(0, 7, 'When this creature enters, draw a card, then discard a card.', {
        kind: 'Triggered',
        label: 'triggered ability',
        wording: 'When this creature enters, you draw a card, then you discard a card.',
        state: {
          differs: { written: 'When this creature enters, you draw a card, then you discard a card.' },
        },
        steps: [
          { depth: 0, text: 'when it enters the battlefield' },
          { depth: 1, text: 'you draw 1 card' },
          { depth: 1, text: 'you discard 1 card of your choice' },
        ],
      }),
      row(
        0,
        68,
        '{2}{U}, {T}: Tap target creature an opponent controls. It doesn’t untap during its controller’s next untap step.',
        {
          kind: 'Activated',
          label: 'activated ability',
          wording: null,
          state: { unwritten: { construct: 'activated ability: skip next untap' } },
          steps: [
            { depth: 0, text: 'cost: {2}{U}, tap this permanent' },
            { depth: 0, text: 'choose a target: a creature an opponent controls' },
            { depth: 1, text: 'tap the target' },
            { depth: 1, text: 'it does not untap during its controller’s next untap step' },
          ],
        },
      ),
    ],
  ),
  'f0000000-0000-4000-8000-000000000002': card(
    'f0000000-0000-4000-8000-000000000002',
    'Ember Archive',
    [
      row(
        0,
        0,
        'At the beginning of your upkeep, exile the top card of your library. You may play it this turn.',
        {
          kind: 'Triggered',
          label: 'triggered ability',
          steps: [
            { depth: 0, text: 'at the beginning of your upkeep' },
            { depth: 1, text: 'exile the top card of your library' },
            { depth: 1, text: 'until end of turn, you may play that card' },
          ],
        },
      ),
      row(0, 98, 'Spells you cast from exile cost {1} less to cast.', {
        state: { by_hand: { reason: 'the engine has no cost reduction for spells cast from exile' } },
        abilities: [],
        wording: null,
        label: BY_HAND,
        steps: [{ depth: 0, text: BY_HAND }],
      }),
    ],
    { state: 'approved', correct: 2 },
  ),
  'f0000000-0000-4000-8000-000000000003': card(
    'f0000000-0000-4000-8000-000000000003',
    'Grove Warden // Grove Awakened',
    [
      row(0, 0, 'Reach', { kind: 'Keyword', label: 'keyword', steps: [{ depth: 0, text: 'has reach' }] }),
      row(1, 0, 'Other creatures you control get +1/+1.', {
        label: 'static ability',
        steps: [{ depth: 0, text: 'other creatures you control get +1/+1' }],
      }),
    ],
    { forge: true, state: 'approved' },
    ['Grove Warden', 'Grove Awakened'],
  ),
}

/** The deck the fixture reads. */
export const READ_QUEUE: ReadQueue = {
  title: 'Izzet Spellcraft',
  cards: Object.values(READ_CARDS).map((value, at) => ({
    oracle_id: value.oracle_id,
    name: value.name,
    read_hash: value.read_hash,
    state: value.state,
    differs: value.sheet.rows.filter((r) => typeof r.state !== 'string' && 'differs' in r.state).length,
    rank: at === 2 ? null : ([14, 1204][at] ?? null),
    in_decks: at !== 1,
  })),
}

/** What the fixture server does differently. */
export interface ReadFixtureOptions {
  /** The deck has nothing left to read. */
  empty?: boolean
  /** The list never arrives. */
  loading?: boolean
  /** The list fails with this code. */
  listFails?: 'network' | 'not_found'
  /** Every verdict answers 409 `stale_read`. */
  stale?: boolean
  /** Milliseconds before each answer. */
  delayMs?: number
}

const never = new Promise<never>(() => undefined)

/** An in-memory read server over the fixture cards: verdicts change the card as the real server would. */
export function fixtureReadsApi(options: ReadFixtureOptions = {}): ReadsApi {
  const cards = new Map(Object.entries(READ_CARDS).map(([id, value]) => [id, structuredClone(value)]))
  const wait = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, options.delayMs ?? 0))
  return {
    async queue(target) {
      if (options.loading === true) return never
      await wait()
      if (options.listFails === 'network') throw new DeckError('network', 0, 'Could not reach the server.')
      if (options.listFails === 'not_found') throw new DeckError('not_found', 404, 'not found')
      const queue =
        target === null
          ? { ...READ_QUEUE, title: '' }
          : 'mine' in target
            ? { title: '', cards: READ_QUEUE.cards.filter((card) => card.in_decks) }
            : READ_QUEUE
      return options.empty === true ? { ...queue, cards: [] } : structuredClone(queue)
    },
    async get(oracleId) {
      await wait()
      const found = cards.get(oracleId)
      if (found === undefined) throw new DeckError('not_found', 404, 'not found')
      return structuredClone(found)
    },
    async judge(oracleId, body) {
      await wait()
      const found = cards.get(oracleId)
      if (found === undefined) throw new DeckError('not_found', 404, 'not found')
      if (options.stale === true || body.read_hash !== found.read_hash) {
        throw new DeckError(
          'stale_read',
          409,
          "the card's draft changed since the page loaded it; read it again",
        )
      }
      const correct = found.correct + (body.verdict === 'correct' ? 1 : 0)
      const wrong = found.wrong + (body.verdict === 'wrong' ? 1 : 0)
      const next: ReadCard = {
        ...found,
        correct,
        wrong,
        state: wrong > 0 ? 'wrong' : correct > 0 ? 'approved' : 'pending',
        mine: {
          verdict: body.verdict,
          note: body.note ?? null,
          created_at: '2026-11-08T12:00:00Z',
          current: true,
        },
      }
      cards.set(oracleId, next)
      return structuredClone(next)
    },
  }
}

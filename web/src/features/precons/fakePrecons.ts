/** Typed fixtures of the precon wire types and a precon server in memory, for tests of the client and the pages. */
import { cardHit, counts, summary } from '@/features/decks/fakeDecks'
import type { Fetch } from '@/net/api'
import { createPreconsApi } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import type {
  Counts,
  DeckZone,
  Issue,
  PreconCardView,
  PreconErrorCode,
  PreconListItem,
  PreconView,
  Reason,
  Support,
} from '@/protocol/generated'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import type { FetchCall } from '@/test-utils/fakeFetch'

/** A verified, playable Commander precon of 100 cards released on 10 August 2018. */
export const preconItem = (over: Partial<PreconListItem> = {}): PreconListItem => ({
  code: 'c18-adaptive-enchantment',
  name: 'Adaptive Enchantment',
  set: 'C18',
  deck_type: 'Commander Deck',
  release_date: '2018-08-10',
  commanders: ['Estrid, the Masked'],
  card_count: 100,
  format: 'commander',
  state: 'verified',
  verified: true,
  playable: true,
  summary: summary(),
  ...over,
})

/** The copies a check found that the game does not run whole. */
export type Found = Partial<Pick<Counts, 'partial' | 'manual' | 'stale' | 'unplayable'>>

/**
 * `item` after a check that found `found`: the counts, the state and the flags that go with them. Unplayable cards
 * block the deck; a deck with `playable` false is blocked too, with or without them.
 */
export function checked(item: PreconListItem, found: Found = {}, playable?: boolean): PreconListItem {
  const { partial = 0, manual = 0, stale = 0, unplayable = 0 } = found
  const total = item.card_count
  const rest = partial + manual + stale + unplayable
  const open = playable ?? unplayable === 0
  const state = !open || unplayable > 0 ? 'blocked' : rest > 0 ? 'manual_play' : 'verified'
  return {
    ...item,
    state,
    verified: state === 'verified',
    playable: open,
    summary: summary({
      format: item.format,
      state,
      counts: counts({ total, verified: total - rest, partial, manual, stale, unplayable }),
    }),
  }
}

/** A clause the game cannot run, on the front face. */
export const reason = (text: string, start = 0, end = 10): Reason => ({ face: 0, span: { start, end }, text })

/** A line of a precon. */
export const cardLine = (
  name: string,
  support: Support = 'verified',
  over: Partial<PreconCardView> = {},
): PreconCardView => ({
  name,
  count: 1,
  printing_id: null,
  card: cardHit(name, { support }),
  support,
  reasons: [],
  ...over,
})

/** A board with the given lines. */
export const board = (zone: DeckZone, cards: PreconCardView[]): PreconView['boards'][number] => ({
  board: zone,
  copies: cards.reduce((sum, card) => sum + card.count, 0),
  cards,
})

/** The details of `item`: its commanders and a main board with the given lines, or one built from its counts. */
export function preconView(
  item: PreconListItem,
  main?: PreconCardView[],
  over: Partial<PreconView> = {},
): PreconView {
  const { counts: found } = item.summary
  const lines: PreconCardView[] = main ?? [
    ...Array.from({ length: found.partial }, (_, i) =>
      cardLine(`Partial Card ${i + 1}`, 'partial', {
        reasons: [reason('the second mode is not implemented')],
      }),
    ),
    ...Array.from({ length: found.manual }, (_, i) =>
      cardLine(`Manual Card ${i + 1}`, 'manual', { reasons: [reason('no rules for this card yet')] }),
    ),
    ...Array.from({ length: found.stale }, (_, i) => cardLine(`Stale Card ${i + 1}`, 'stale')),
    ...Array.from({ length: found.unplayable }, (_, i) => cardLine(`Unplayable Card ${i + 1}`, 'unplayable')),
    cardLine('Forest', 'verified', { count: Math.max(found.verified - item.commanders.length, 1) }),
  ]
  const commanders = item.commanders.map((name) => cardLine(name))
  return {
    precon: item,
    issues: [],
    boards: [...(commanders.length > 0 ? [board('commander', commanders)] : []), board('main', lines)],
    ...over,
  }
}

/** A way a game refuses a deck. */
export const issue = (message: string, over: Partial<Issue> = {}): Issue => ({
  code: 'copies',
  message,
  zone: 'main',
  oracle_id: null,
  name: null,
  ...over,
})

/** What the fake precon server does beyond the usual; leave a field out for the usual good behaviour. */
export interface PreconsScript {
  /** The list, newest first. */
  precons?: PreconListItem[]
  /** The details by code; a precon of the list without one gets a view built from its counts. */
  views?: PreconView[]
  /** Every call fails with this code (and status). */
  fail?: { code: PreconErrorCode; status: number }
  /** Changes the answer of a call, or lets it through with `undefined`. */
  override?: (call: FetchCall) => Response | Promise<Response> | undefined
}

/** A precon server in memory: the client over it and every request it saw. */
export interface FakePrecons {
  api: PreconsApi
  fetch: Fetch
  calls: FetchCall[]
}

/** Starts a precon server in memory that answers as `mtg-server` does. */
export function startPrecons(script: PreconsScript = {}): FakePrecons {
  const precons = script.precons ?? [preconItem()]
  const { fetch, calls } = fakeFetch((call) => {
    const custom = script.override?.(call)
    if (custom !== undefined) return custom
    if (script.fail !== undefined) {
      return jsonResponse(script.fail.status, {
        error: script.fail.code,
        message: `fake ${script.fail.code}`,
      })
    }
    const path = new URL(call.url, 'http://localhost').pathname
    if (path === '/api/precons') return jsonResponse(200, { precons })
    const code = decodeURIComponent(path.replace('/api/precons/', ''))
    const found = precons.find((item) => item.code === code)
    if (found === undefined)
      return jsonResponse(404, { error: 'not_found', message: 'no precon has that code' })
    return jsonResponse(200, script.views?.find((view) => view.precon.code === code) ?? preconView(found))
  })
  return { api: createPreconsApi({ fetch }), fetch, calls }
}

/**
 * The deck-group endpoints of `mtg-server` (docs/server.md, Deck groups): list the account's groups, create, rename,
 * collapse, reorder and delete one, and file a deck in a group or none. The bodies are the generated types; every
 * answer is checked at runtime and every failure is a `DeckError`, since the endpoints share the deck endpoints'
 * codes. Every request carries the session cookie (`credentials: 'include'`).
 */
import type {
  DeckGroupChange,
  DeckGroupChoice,
  DeckGroupOrder,
  DeckGroupView,
  NewDeckGroup,
} from '@/protocol/generated'
import type { Fetch } from './api'
import { isFlag, isListOf, isRecord, isText } from './check'
import { DeckError, isDeckErrorCode } from './decks'

/** Options for `createDeckGroupsApi`. */
export interface DeckGroupsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin. */
  baseUrl?: string
}

/** The group calls; each rejects with a `DeckError`. */
export interface DeckGroupsApi {
  /** `GET /api/deck-groups`: the groups in the owner's order, each with its deck ids. */
  list(): Promise<DeckGroupView[]>
  /** `POST /api/deck-groups`: a new open group after the others. */
  create(name: string): Promise<DeckGroupView>
  /** `PATCH /api/deck-groups/{id}`: renames, collapses or opens the group. */
  change(id: string, change: DeckGroupChange): Promise<DeckGroupView>
  /** `DELETE /api/deck-groups/{id}`: deletes the group; its decks are ungrouped. */
  remove(id: string): Promise<void>
  /** `PUT /api/deck-groups/order`: every group id once, in the new order. */
  reorder(ids: string[]): Promise<void>
  /** `PUT /api/decks/{id}/group`: files the deck in the group, or in none for `null`. */
  file(deckId: string, group: string | null): Promise<void>
}

/** A group as the server sends it, or null for anything else. */
export function readGroup(body: unknown): DeckGroupView | null {
  if (
    !isRecord(body) ||
    !isText(body.id) ||
    !isText(body.name) ||
    !isFlag(body.collapsed) ||
    !isListOf(body.decks, isText)
  ) {
    return null
  }
  return { id: body.id, name: body.name, collapsed: body.collapsed, decks: body.decks }
}

function readList(body: unknown): DeckGroupView[] | null {
  if (!isRecord(body) || !Array.isArray(body.groups)) return null
  const groups = body.groups.map(readGroup)
  return groups.every((group) => group !== null) ? groups : null
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function failure(response: Response, body: unknown): DeckError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    return new DeckError(isDeckErrorCode(body.error) ? body.error : 'unknown', response.status, body.message)
  }
  return new DeckError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status} and no readable error.`,
  )
}

/** Builds the client; call it once, or use the shared `deckGroupsApi`. */
export function createDeckGroupsApi(options: DeckGroupsOptions = {}): DeckGroupsApi {
  const base = options.baseUrl ?? ''
  const groups = `${base}/api/deck-groups`

  async function call<T>(url: string, read: (body: unknown) => T | null, init: RequestInit): Promise<T> {
    const send: Fetch = options.fetch ?? ((target, request) => globalThis.fetch(target, request))
    let response: Response
    let text: string
    try {
      response = await send(url, { credentials: 'include', ...init })
      text = await response.text()
    } catch {
      throw new DeckError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    const body = parseJson(text)
    if (!response.ok) throw failure(response, body)
    const value = read(body)
    if (value === null) {
      throw new DeckError('bad_response', response.status, 'The server sent an answer this page cannot read.')
    }
    return value
  }
  const json = (method: string, body: unknown): RequestInit => ({
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  const done = () => true

  return {
    list: () => call(groups, readList, {}),
    create: (name) => call(groups, readGroup, json('POST', { name } satisfies NewDeckGroup)),
    change: (id, change) =>
      call(`${groups}/${encodeURIComponent(id)}`, readGroup, json('PATCH', change satisfies DeckGroupChange)),
    remove: async (id) => {
      await call(`${groups}/${encodeURIComponent(id)}`, done, { method: 'DELETE' })
    },
    reorder: async (ids) => {
      await call(`${groups}/order`, done, json('PUT', { ids } satisfies DeckGroupOrder))
    },
    file: async (deckId, group) => {
      await call(
        `${base}/api/decks/${encodeURIComponent(deckId)}/group`,
        done,
        json('PUT', { group } satisfies DeckGroupChoice),
      )
    },
  }
}

/** The client for the page origin, using the global `fetch`. */
export const deckGroupsApi: DeckGroupsApi = createDeckGroupsApi()

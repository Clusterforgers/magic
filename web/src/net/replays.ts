/**
 * `GET /api/games/{id}/replay` of `mtg-server` (docs/server.md, Replays): a finished game step by step. The session
 * cookie rides along, so the account that held a seat gets its own hidden cards; anybody else the public view.
 */
import type { Replay } from '@/protocol/generated'
import type { Fetch } from './api'
import { isListOf, isRecord, isText } from './check'

/** Why a replay did not load: none to show (an unknown game, one that has not ended), the network, or anything else. */
export type ReplayFailure = 'not_found' | 'network' | 'failed'

/** A replay that did not load, with its reason. */
export class ReplayError extends Error {
  readonly reason: ReplayFailure

  constructor(reason: ReplayFailure) {
    super(reason)
    this.name = 'ReplayError'
    this.reason = reason
  }
}

/** The replay body, checked as far as the page relies on it: names, and steps whose first is at `from_seq` 0. */
export function readReplay(body: unknown): Replay | null {
  if (!isRecord(body) || !isListOf(body.names, isText) || !Array.isArray(body.steps)) return null
  const first: unknown = body.steps[0]
  if (!isRecord(first) || first.from_seq !== 0) return null
  if (
    !body.steps.every((step: unknown) => isRecord(step) && Array.isArray(step.events) && isRecord(step.table))
  )
    return null
  return body as Replay
}

/** Loads the replay of `game`; `fetch` defaults to the global one. */
export async function loadReplay(game: string, fetch?: Fetch, signal?: AbortSignal): Promise<Replay> {
  const send: Fetch = fetch ?? ((url, request) => globalThis.fetch(url, request))
  let response: Response
  let text: string
  try {
    response = await send(`/api/games/${encodeURIComponent(game)}/replay`, { credentials: 'include', signal })
    text = await response.text()
  } catch {
    throw new ReplayError('network')
  }
  if (response.status === 404) throw new ReplayError('not_found')
  let body: unknown = null
  try {
    body = JSON.parse(text)
  } catch {
    body = null
  }
  const replay = response.ok ? readReplay(body) : null
  if (replay === null) throw new ReplayError('failed')
  return replay
}

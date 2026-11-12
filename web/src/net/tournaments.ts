/**
 * The tournament endpoints of `mtg-server` (docs/server.md, Tournaments): the player's list, creating one, signing up
 * by invite and the organiser's controls. Answers are the generated types, checked at runtime; every failure is a
 * `TournamentError` with the server's code. Every request carries the session cookie (`credentials: 'include'`).
 */
import type {
  CreateTournament,
  JoinTournament,
  MatchOutcome,
  TournamentCreated,
  TournamentErrorCode,
  TournamentInvite,
  TournamentList,
  TournamentView,
} from '@/protocol/generated'
import type { Fetch } from './api'
import { isCount, isListOf, isRecord, isText } from './check'

/** The server's codes, plus the client's own for a lost connection and an answer it cannot read. */
export type TournamentFailure = TournamentErrorCode | 'network' | 'bad_response' | 'unknown'

const CODES: readonly TournamentErrorCode[] = [
  'invalid_request',
  'unauthenticated',
  'forbidden',
  'not_found',
  'started',
  'not_running',
  'already_joined',
  'full',
  'not_enough_entrants',
  'undecided',
  'format_unavailable',
  'accounts_unavailable',
  'unavailable',
]

function isCode(text: string): text is TournamentErrorCode {
  return (CODES as readonly string[]).includes(text)
}

/** A failed tournament request: the code for programs, the message for people. */
export class TournamentError extends Error {
  /** The machine-readable code. */
  readonly code: TournamentFailure
  /** The HTTP status; 0 when the server was not reached. */
  readonly status: number

  constructor(code: TournamentFailure, status: number, message: string) {
    super(message)
    this.name = 'TournamentError'
    this.code = code
    this.status = status
  }
}

/** Options for `createTournamentsApi`. */
export interface TournamentsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin. */
  baseUrl?: string
}

/** The tournament calls; each rejects with a `TournamentError`. */
export interface TournamentsApi {
  /** `GET /api/tournaments`: the tournaments the player organises or plays in, newest first. */
  list(): Promise<TournamentList>
  /** `POST /api/tournaments`: a new tournament in sign-up. */
  create(body: CreateTournament): Promise<TournamentCreated>
  /** `GET /api/tournaments/{id}`: one tournament with its rounds and standings. */
  get(id: string): Promise<TournamentView>
  /** `GET /api/tournaments/invite/{code}`: what an invite signs up for. */
  invite(code: string): Promise<TournamentInvite>
  /** `POST /api/tournaments/invite/{code}/join`: signs the player up. */
  join(code: string, body: JoinTournament): Promise<TournamentView>
  /** `POST /api/tournaments/{id}/{action}`: an organiser's step or the player's drop. */
  act(id: string, action: 'start' | 'next' | 'games' | 'drop'): Promise<TournamentView>
  /** `POST /api/tournaments/{id}/leave`: leaves before the start. */
  leave(id: string): Promise<void>
  /** `POST /api/tournaments/{id}/entrants/{n}/drop`: the organiser drops an entrant. */
  dropEntrant(id: string, entrant: number): Promise<TournamentView>
  /** `PUT /api/tournaments/{id}/matches/{round}/{table}`: the organiser sets a result. */
  setOutcome(id: string, round: number, table: number, outcome: MatchOutcome): Promise<TournamentView>
  /** `DELETE /api/tournaments/{id}`: the organiser removes the tournament. */
  remove(id: string): Promise<void>
}

/** True for a tournament view: the fields the pages read are checked. */
export function isTournamentView(body: unknown): body is TournamentView {
  return (
    isRecord(body) &&
    isText(body.id) &&
    isText(body.name) &&
    isText(body.status) &&
    isCount(body.round) &&
    isListOf(body.entrants, isRecord) &&
    isListOf(body.rounds_played, isRecord) &&
    isListOf(body.standings, isRecord)
  )
}

function isList(body: unknown): body is TournamentList {
  return isRecord(body) && isListOf(body.tournaments, isRecord)
}

function isCreated(body: unknown): body is TournamentCreated {
  return isRecord(body) && isText(body.id) && isText(body.invite)
}

function isInvite(body: unknown): body is TournamentInvite {
  return isRecord(body) && isText(body.id) && isText(body.name) && isListOf(body.entrants, isRecord)
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return undefined
  }
}

function failure(response: Response, body: unknown): TournamentError {
  if (isRecord(body) && isText(body.error) && isText(body.message)) {
    return new TournamentError(isCode(body.error) ? body.error : 'unknown', response.status, body.message)
  }
  return new TournamentError(
    'bad_response',
    response.status,
    `The server answered with HTTP ${response.status} and no readable error.`,
  )
}

/** Builds the client; call it once, or use the shared `tournamentsApi`. */
export function createTournamentsApi(options: TournamentsOptions = {}): TournamentsApi {
  const base = `${options.baseUrl ?? ''}/api/tournaments`
  const of = (id: string): string => `${base}/${encodeURIComponent(id)}`

  async function call<T>(
    url: string,
    check: (body: unknown) => body is T,
    init: RequestInit = {},
  ): Promise<T> {
    const send: Fetch = options.fetch ?? ((target, request) => globalThis.fetch(target, request))
    let response: Response
    let text: string
    try {
      response = await send(url, { credentials: 'include', ...init })
      text = await response.text()
    } catch {
      throw new TournamentError(
        'network',
        0,
        'Could not reach the server. Check your connection and try again.',
      )
    }
    const body = response.status === 204 ? null : parseJson(text)
    if (!response.ok) throw failure(response, body)
    if (!check(body)) {
      throw new TournamentError(
        'bad_response',
        response.status,
        'The server sent an answer this page cannot read.',
      )
    }
    return body
  }

  const json = (method: string, body: unknown): RequestInit => ({
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  const empty = (body: unknown): body is null => body === null

  return {
    list: () => call(base, isList),
    create: (body) => call(base, isCreated, json('POST', body)),
    get: (id) => call(of(id), isTournamentView),
    invite: (code) => call(`${base}/invite/${encodeURIComponent(code)}`, isInvite),
    join: (code, body) =>
      call(`${base}/invite/${encodeURIComponent(code)}/join`, isTournamentView, json('POST', body)),
    act: (id, action) => call(`${of(id)}/${action}`, isTournamentView, { method: 'POST' }),
    leave: async (id) => {
      await call(`${of(id)}/leave`, empty, { method: 'POST' })
    },
    dropEntrant: (id, entrant) =>
      call(`${of(id)}/entrants/${entrant}/drop`, isTournamentView, { method: 'POST' }),
    setOutcome: (id, round, table, outcome) =>
      call(`${of(id)}/matches/${round}/${table}`, isTournamentView, json('PUT', { outcome })),
    remove: async (id) => {
      await call(of(id), empty, { method: 'DELETE' })
    },
  }
}

/** The client for the page origin, using the global `fetch`. */
export const tournamentsApi: TournamentsApi = createTournamentsApi()

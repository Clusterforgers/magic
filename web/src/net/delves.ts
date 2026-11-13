/**
 * The Delves endpoints of `mtg-server` (docs/server.md, Delves): the caller's runs, a new run, joining by code, the
 * run brought up to date, one action of a member, and the campaign content the pages draw.
 */
import type { CreateDelve, DelveAction, DelveContent, DelveList, DelveView } from '@/protocol/generated'
import type { Fetch } from './api'
import { isRecord } from './check'

/** Why a call failed: the server's code, or `network` when it was not reached. */
export class DelveError extends Error {
  readonly code: string
  readonly status: number

  constructor(code: string, status: number, message: string) {
    super(message)
    this.name = 'DelveError'
    this.code = code
    this.status = status
  }
}

export interface DelvesApi {
  list(): Promise<DelveList>
  create(body: CreateDelve): Promise<DelveView>
  join(code: string): Promise<DelveView>
  get(id: string): Promise<DelveView>
  act(id: string, action: DelveAction): Promise<DelveView>
  content(): Promise<DelveContent>
}

function isView(body: unknown): body is DelveView {
  return isRecord(body) && typeof body.id === 'string' && isRecord(body.run)
}

function isList(body: unknown): body is DelveList {
  return isRecord(body) && Array.isArray(body.delves)
}

function isContent(body: unknown): body is DelveContent {
  return isRecord(body) && Array.isArray(body.cards) && Array.isArray(body.bosses)
}

/** Builds the client; call it once, or use the shared `delvesApi`. */
export function createDelvesApi(options: { fetch?: Fetch; baseUrl?: string } = {}): DelvesApi {
  const base = `${options.baseUrl ?? ''}/api/delves`

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
      throw new DelveError('network', 0, 'Could not reach the server. Check your connection and try again.')
    }
    let body: unknown = null
    try {
      body = text === '' ? null : JSON.parse(text)
    } catch {
      body = null
    }
    if (!response.ok) {
      const code = isRecord(body) && typeof body.error === 'string' ? body.error : 'unknown'
      const message =
        isRecord(body) && typeof body.message === 'string'
          ? body.message
          : `The server answered with HTTP ${response.status}.`
      throw new DelveError(code, response.status, message)
    }
    if (!check(body))
      throw new DelveError(
        'bad_response',
        response.status,
        'The server sent an answer this page cannot read.',
      )
    return body
  }

  const post = (body: unknown): RequestInit => ({
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })

  return {
    list: () => call(base, isList),
    create: (body) => call(base, isView, post(body)),
    join: (code) => call(`${base}/join/${encodeURIComponent(code.trim())}`, isView, { method: 'POST' }),
    get: (id) => call(`${base}/${encodeURIComponent(id)}`, isView),
    act: (id, action) => call(`${base}/${encodeURIComponent(id)}/act`, isView, post(action)),
    content: () => call(`${base}/content`, isContent),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const delvesApi: DelvesApi = createDelvesApi()

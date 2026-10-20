// Goal: choosing a deck sends PUT /api/games/{id}/seats/{n}/deck with the deck id as JSON, the seat's token and the session, reads only a well-formed SeatDeck, and turns every failure into a LobbyError whose code, status and issues are the server's (a refused deck keeps its issues and drops any that are not issues), with a sentence in plain words for each deck code; a join may carry a deck; and the roster of the socket accepts a deck only when it has the three facts.
import { describe, expect, it } from 'vitest'
import { fakeFetch, jsonResponse } from '@/test-utils/fakeFetch'
import { createApi } from './api'
import {
  DECK_CHOICE_ERROR_CODES,
  describeLobbyError,
  isDeckChoiceErrorCode,
  isLobbyErrorCode,
  LobbyError,
  lobbyFailure,
} from './lobbyErrors'
import { parseFrame } from './parse'

const GAME = '6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55'
const TOKEN = 'a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90'
const BURN = { name: 'Burn', format: 'modern', manual_cards: 3 }

async function failureOf(promise: Promise<unknown>): Promise<LobbyError> {
  try {
    await promise
  } catch (error) {
    if (error instanceof LobbyError) return error
    throw error
  }
  throw new Error('the call succeeded but should have failed')
}

const issue = {
  code: 'copies',
  message: '5 copies of Lightning Bolt',
  zone: 'main',
  oracle_id: 'o-1',
  name: 'Lightning Bolt',
}

describe('chooseDeck', () => {
  it('puts the deck id as JSON with the seat token as a bearer token and the session cookie, and reads the seat’s deck', async () => {
    const fake = fakeFetch(() => jsonResponse(200, BURN))
    const deck = await createApi({ fetch: fake.fetch }).chooseDeck(GAME, 2, 'a-stored-id', { token: TOKEN })
    expect(deck).toEqual(BURN)
    expect(fake.calls).toHaveLength(1)
    expect(fake.calls[0]?.url).toBe(`/api/games/${GAME}/seats/2/deck`)
    expect(fake.calls[0]?.init).toEqual({
      method: 'PUT',
      credentials: 'include',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${TOKEN}` },
      body: JSON.stringify({ deck: 'a-stored-id' }),
    })
  })

  it('relies on the session alone when it has no token', async () => {
    const fake = fakeFetch(() => jsonResponse(200, BURN))
    await createApi({ fetch: fake.fetch }).chooseDeck(GAME, 0, 'mono-red')
    expect(fake.calls[0]?.init?.headers).toEqual({ 'content-type': 'application/json' })
    expect(fake.calls[0]?.init?.credentials).toBe('include')
  })

  it('escapes the game id in the path', async () => {
    const fake = fakeFetch(() => jsonResponse(200, BURN))
    await createApi({ fetch: fake.fetch }).chooseDeck('a/b c', 1, 'mono-red')
    expect(fake.calls[0]?.url).toBe('/api/games/a%2Fb%20c/seats/1/deck')
  })

  it.each<[string, number]>([
    ['a negative seat', -1],
    ['a fraction', 1.5],
    ['not a number', Number.NaN],
  ])('sends nothing for %s', async (_what, seat) => {
    const fake = fakeFetch(() => jsonResponse(200, BURN))
    const error = await failureOf(createApi({ fetch: fake.fetch }).chooseDeck(GAME, seat, 'mono-red'))
    expect([error.code, error.status]).toEqual(['invalid_request', 0])
    expect(fake.calls).toEqual([])
  })

  it.each<[string, unknown]>([
    ['no body', undefined],
    ['null', null],
    ['a list', []],
    ['a name that is not text', { ...BURN, name: 7 }],
    ['no format', { name: 'Burn', manual_cards: 3 }],
    ['a count below zero', { ...BURN, manual_cards: -1 }],
    ['a count that is not whole', { ...BURN, manual_cards: 1.5 }],
  ])('is a bad_response for %s', async (_what, body) => {
    const fake = fakeFetch(() => jsonResponse(200, body))
    const error = await failureOf(createApi({ fetch: fake.fetch }).chooseDeck(GAME, 0, 'mono-red'))
    expect(error.code).toBe('bad_response')
  })

  it('is a bad_response for the wrong success status', async () => {
    const fake = fakeFetch(() => jsonResponse(201, BURN))
    const error = await failureOf(createApi({ fetch: fake.fetch }).chooseDeck(GAME, 0, 'mono-red'))
    expect([error.code, error.status]).toEqual(['bad_response', 201])
  })

  it.each<[number, string]>([
    [403, 'forbidden'],
    [404, 'not_found'],
    [404, 'unknown_deck'],
    [409, 'seat_ready'],
    [409, 'started'],
    [401, 'unauthenticated'],
    [503, 'unavailable'],
    [500, 'internal'],
  ])('keeps HTTP %i %s as the code', async (status, code) => {
    const fake = fakeFetch(() => jsonResponse(status, { error: code, message: 'in the server words' }))
    const error = await failureOf(createApi({ fetch: fake.fetch }).chooseDeck(GAME, 0, 'mono-red'))
    expect({ code: error.code, status: error.status, message: error.message }).toEqual({
      code,
      status,
      message: 'in the server words',
    })
    expect(error.issues).toEqual([])
  })

  it('keeps the issues of a refused deck, and leaves out what is not an issue', async () => {
    const fake = fakeFetch(() =>
      jsonResponse(422, {
        error: 'deck_blocked',
        message: 'blocked',
        issues: [
          issue,
          { code: 'copies' },
          'text',
          null,
          { ...issue, zone: null, oracle_id: null, name: null },
        ],
      }),
    )
    const error = await failureOf(createApi({ fetch: fake.fetch }).chooseDeck(GAME, 0, 'stored'))
    expect([error.code, error.status]).toEqual(['deck_blocked', 422])
    expect(error.issues).toEqual([issue, { ...issue, zone: null, oracle_id: null, name: null }])
  })

  it.each<[string, unknown]>([
    ['no issues', undefined],
    ['issues that are not a list', 'many'],
    ['an empty list', []],
  ])('has no issues for a refused deck with %s', async (_what, issues) => {
    const fake = fakeFetch(() => jsonResponse(422, { error: 'deck_blocked', message: 'blocked', issues }))
    const error = await failureOf(createApi({ fetch: fake.fetch }).chooseDeck(GAME, 0, 'stored'))
    expect(error.issues).toEqual([])
  })

  it('reports an offline browser as network', async () => {
    const error = await failureOf(
      createApi({ fetch: () => Promise.reject(new TypeError('Failed to fetch')) }).chooseDeck(GAME, 0, 'x'),
    )
    expect(error.code).toBe('network')
  })
})

describe('the deck codes', () => {
  it('are the three the server adds, apart from the lobby codes', () => {
    expect(DECK_CHOICE_ERROR_CODES).toEqual(['unknown_deck', 'deck_blocked', 'seat_ready'])
    for (const code of DECK_CHOICE_ERROR_CODES) {
      expect(isDeckChoiceErrorCode(code)).toBe(true)
      expect(isLobbyErrorCode(code)).toBe(false)
    }
    expect(isDeckChoiceErrorCode('full')).toBe(false)
    expect(isDeckChoiceErrorCode('__proto__')).toBe(false)
  })

  it('calls a deck code it does not know unknown, and keeps issues only for deck_blocked', () => {
    const odd = lobbyFailure(new Response(null, { status: 422 }), {
      error: 'deck_odd',
      message: 'm',
      issues: [issue],
    })
    expect([odd.code, odd.issues]).toEqual(['unknown', []])
    const other = lobbyFailure(new Response(null, { status: 409 }), {
      error: 'seat_ready',
      message: 'm',
      issues: [issue],
    })
    expect(other.issues).toEqual([])
  })

  it.each([...DECK_CHOICE_ERROR_CODES])(
    'has a sentence of its own for %s, in the general words and about a deck',
    (code) => {
      const error = new LobbyError(code, 409, 'server message')
      const general = describeLobbyError(error)
      const about = describeLobbyError(error, 'deck')
      for (const text of [general, about]) {
        expect(text).toMatch(/[.!?]$/)
        expect(text.length).toBeGreaterThan(20)
        expect(text).not.toMatch(/[a-z]+_[a-z]+/)
      }
      if (code !== 'deck_blocked') expect(about).not.toBe(general)
    },
  )

  it.each<[Parameters<typeof describeLobbyError>[1], 'forbidden' | 'not_found' | 'started', RegExp]>([
    ['deck', 'forbidden', /your own seat/],
    ['deck', 'not_found', /no longer have a seat/],
    ['deck', 'started', /deck can no longer change/],
  ])('says %s / %s about the deck', (about, code, words) => {
    expect(describeLobbyError(new LobbyError(code, 409, 'm'), about)).toMatch(words)
  })
})

describe('joining with a deck', () => {
  it.each<[string, (api: ReturnType<typeof createApi>) => Promise<unknown>, unknown]>([
    [
      'joinGame',
      (api) => api.joinGame(GAME, { name: 'Bo', deck: 'mono-red' }),
      { name: 'Bo', deck: 'mono-red' },
    ],
    ['joinGame without a name', (api) => api.joinGame(GAME, { deck: 'x' }), { deck: 'x' }],
    ['joinGame without a deck', (api) => api.joinGame(GAME, { name: 'Bo' }), { name: 'Bo' }],
    ['joinByCode', (api) => api.joinByCode('ABCD2345', { deck: 'x' }), { code: 'ABCD2345', deck: 'x' }],
  ])('sends the deck only when there is one (%s)', async (_what, call, body) => {
    const fake = fakeFetch(() => jsonResponse(200, { game: GAME, seat: 1, token: 't' }))
    await call(createApi({ fetch: fake.fetch }))
    expect(JSON.parse(String(fake.calls[0]?.init?.body))).toEqual(body)
  })
})

describe('the roster of the socket', () => {
  const frame = (deck: unknown) =>
    JSON.stringify({
      type: 'Lobby',
      seats: [
        { seat: 0, name: 'Ann', connected: true, ready: false, ...(deck === undefined ? {} : { deck }) },
      ],
      needed: 1,
    })

  it.each<[string, unknown]>([
    ['no deck', undefined],
    ['a deck', BURN],
  ])('reads a seat with %s', (_what, deck) => {
    expect(parseFrame(frame(deck)).ok).toBe(true)
  })

  it.each<[string, unknown]>([
    ['null', null],
    ['text', 'Burn'],
    ['a list', []],
    ['no name', { format: 'modern', manual_cards: 3 }],
    ['a name that is not text', { ...BURN, name: 3 }],
    ['no format', { name: 'Burn', manual_cards: 3 }],
    ['a count that is not a number', { ...BURN, manual_cards: '3' }],
    ['a negative count', { ...BURN, manual_cards: -3 }],
  ])('refuses a seat whose deck is %s', (_what, deck) => {
    expect(parseFrame(frame(deck)).ok).toBe(false)
  })
})

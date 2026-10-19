// Goal: every way a deck call can fail has plain words that never show the bare code, carry the server's sentence where it names the problem, and tell a server without decks and an ended session apart from other failures.
import { describe, expect, it } from 'vitest'
import { DECK_ERROR_CODES, DeckError } from '@/net/decks'
import type { DeckFailureCode } from '@/net/decks'
import { ApiError } from '@/net/api'
import { describeDeckError, isDecksUnavailable, isSessionEnded } from './errors'

const ALL: readonly DeckFailureCode[] = [...DECK_ERROR_CODES, 'unknown', 'network', 'bad_response']

describe('describeDeckError', () => {
  it.each(ALL)('words %s as a sentence for a person', (code) => {
    const words = describeDeckError(new DeckError(code, 400, 'name is blank'))
    expect(words.length).toBeGreaterThan(20)
    expect(words).toMatch(/[.!?"]$/)
    expect(words).not.toContain(code)
    expect(words).not.toMatch(/undefined|\[object/)
  })

  it.each(['invalid_request', 'invalid_deck', 'unknown_cards', 'export_failed'] as const)(
    'carries the server sentence of %s, capitalised and ended',
    (code) => {
      expect(describeDeckError(new DeckError(code, 400, 'the name must be 1 to 100 characters'))).toContain(
        'The name must be 1 to 100 characters.',
      )
    },
  )

  it('does not double the full stop of a server sentence', () => {
    expect(describeDeckError(new DeckError('invalid_deck', 400, 'Too many cards.'))).toMatch(
      /Too many cards\.$/,
    )
  })

  it('says what to do about the deck limit', () => {
    expect(describeDeckError(new DeckError('deck_limit', 409, 'limit'))).toMatch(/delete one/i)
  })

  it('says the session ended when the server answers 401', () => {
    expect(describeDeckError(new DeckError('unauthenticated', 401, 'sign in'))).toMatch(/sign in again/i)
  })

  it('quotes what an unknown refusal said', () => {
    expect(describeDeckError(new DeckError('unknown', 418, 'short and stout'))).toContain('"short and stout"')
  })

  it.each([
    [502, ' (status 502)'],
    [0, ''],
  ])('names the status of an unreadable answer when there is one (%i)', (status, shown) => {
    expect(describeDeckError(new DeckError('bad_response', status, 'x'))).toContain(
      `answer this page could not read${shown}.`,
    )
  })

  it.each([
    ['an ApiError of the game endpoints', new ApiError('network', 0, 'offline')],
    ['a plain Error', new Error('boom')],
    ['a string', 'boom'],
    ['nothing', undefined],
  ])('gives %s the general apology', (_what, error) => {
    expect(describeDeckError(error)).toBe('Something unexpected went wrong. Try again.')
  })
})

describe('the failures a page treats on their own', () => {
  it('recognises a server without decks only by its code', () => {
    expect(isDecksUnavailable(new DeckError('decks_unavailable', 503, 'no database'))).toBe(true)
    expect(isDecksUnavailable(new DeckError('cards_unavailable', 503, 'no cards'))).toBe(false)
    expect(isDecksUnavailable(new DeckError('unavailable', 503, 'restarting'))).toBe(false)
    expect(isDecksUnavailable(new Error('decks_unavailable'))).toBe(false)
  })

  it('recognises an ended session only by its code', () => {
    expect(isSessionEnded(new DeckError('unauthenticated', 401, 'sign in'))).toBe(true)
    expect(isSessionEnded(new DeckError('forbidden_origin', 403, 'origin'))).toBe(false)
    expect(isSessionEnded('unauthenticated')).toBe(false)
  })
})

// Goal: a refused mark is told in plain words, the cap of marks and the budget of changes each with its own way out.
import { describe, expect, it } from 'vitest'
import { DeckError } from '@/net/decks'
import { describeMarkError } from './errors'

describe('describeMarkError', () => {
  it.each([
    ['fix_mark_limit', 409, /take a mark back/i],
    ['rate_limited', 429, /wait a few minutes/i],
    ['not_found', 404, /does not know this card/i],
    ['unauthenticated', 401, /sign in again/i],
  ] as const)('words %s', (code, status, words) => {
    expect(describeMarkError(new DeckError(code, status, 'server words'))).toMatch(words)
  })
})

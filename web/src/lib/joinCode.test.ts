// Goal: a join code is shown in two groups of four and linked by its raw eight symbols, and the tab keeps it with the invites of its own game only, reading back nothing for another game, a blank or a mistyped code, and never failing when storage does.
import { describe, expect, it } from 'vitest'
import { codeText, joinLinkUrl } from './joinCode'
import { loadInvites, saveInvites } from './invites'
import type { KeyValueStore } from './invites'

const GAME = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'
const seats = [{ seat: 0, name: 'Ana', token: 'token-0' }]

function memoryStore(): KeyValueStore & { data: Record<string, string> } {
  const data: Record<string, string> = {}
  return {
    data,
    getItem: (key) => data[key] ?? null,
    setItem: (key, value) => {
      data[key] = value
    },
  }
}

describe('how a code reads', () => {
  it.each([
    ['K7M29QXA', 'K7M2-9QXA'],
    ['ABCDEFGH', 'ABCD-EFGH'],
    ['SHORT', 'SHORT'],
    ['', ''],
    ['TOOLONGCODE', 'TOOLONGCODE'],
  ])('shows %s as %s', (code, shown) => {
    expect(codeText(code)).toBe(shown)
  })

  it('links the raw code on the page origin, escaped', () => {
    expect(joinLinkUrl('https://play.example', 'K7M29QXA')).toBe('https://play.example/join/K7M29QXA')
    expect(joinLinkUrl('http://localhost:5173', 'a/b?c')).toBe('http://localhost:5173/join/a%2Fb%3Fc')
  })
})

describe('keeping the code with the invites', () => {
  it('gives back the code with the seats of the game it was saved for', () => {
    const store = memoryStore()
    expect(saveInvites({ game: GAME, code: 'K7M29QXA', seats }, store)).toBe(true)
    expect(loadInvites(GAME, store)).toEqual({ game: GAME, code: 'K7M29QXA', seats })
  })

  it('gives back no code for a game saved without one', () => {
    const store = memoryStore()
    saveInvites({ game: GAME, seats }, store)
    expect(loadInvites(GAME, store)).toEqual({ game: GAME, seats })
    expect('code' in (loadInvites(GAME, store) ?? {})).toBe(false)
  })

  it('gives back nothing for another game', () => {
    const store = memoryStore()
    saveInvites({ game: GAME, code: 'K7M29QXA', seats }, store)
    expect(loadInvites('another', store)).toBeNull()
  })

  it.each([
    ['a number', 7],
    ['null', null],
    ['an empty string', ''],
    ['a list', ['K7M29QXA']],
  ])('ignores a code that is %s and keeps the seats', (_what, code) => {
    const store = memoryStore()
    store.setItem('magic.invites', JSON.stringify({ game: GAME, code, seats }))
    expect(loadInvites(GAME, store)).toEqual({ game: GAME, seats })
  })

  it('keeps the code of the newest game only', () => {
    const store = memoryStore()
    saveInvites({ game: GAME, code: 'K7M29QXA', seats }, store)
    saveInvites({ game: 'newer', code: 'ABCDEFGH', seats }, store)
    expect(loadInvites(GAME, store)).toBeNull()
    expect(loadInvites('newer', store)?.code).toBe('ABCDEFGH')
  })

  it('says it could not save, and reads nothing, when the storage throws', () => {
    const throwing: KeyValueStore = {
      getItem: () => {
        throw new DOMException('blocked', 'SecurityError')
      },
      setItem: () => {
        throw new DOMException('quota', 'QuotaExceededError')
      },
    }
    expect(saveInvites({ game: GAME, code: 'K7M29QXA', seats }, throwing)).toBe(false)
    expect(loadInvites(GAME, throwing)).toBeNull()
  })
})

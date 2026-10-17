// Goal: the remembered invites round-trip for their own game, read back as nothing for anything else or anything damaged, and a storage that throws is never allowed to break the caller.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { loadInvites, saveInvites } from './invites'
import type { Invites, KeyValueStore } from './invites'

const GAME = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'
const invites: Invites = {
  game: GAME,
  seats: [
    { seat: 0, name: 'Ana', token: 'token-0' },
    { seat: 1, name: 'Bo', token: 'token-1' },
  ],
}

/** A store that keeps what it is given, like `sessionStorage` does for one tab. */
function memoryStore(initial: Record<string, string> = {}): KeyValueStore & { data: Record<string, string> } {
  const data = { ...initial }
  return {
    data,
    getItem: (key) => data[key] ?? null,
    setItem: (key, value) => {
      data[key] = value
    },
  }
}

const throwing: KeyValueStore = {
  getItem: () => {
    throw new DOMException('blocked', 'SecurityError')
  },
  setItem: () => {
    throw new DOMException('quota', 'QuotaExceededError')
  },
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('saving and loading invites', () => {
  it('gives back what was saved for the same game', () => {
    const store = memoryStore()
    expect(saveInvites(invites, store)).toBe(true)
    expect(loadInvites(GAME, store)).toEqual(invites)
  })

  it('keeps only the newest game', () => {
    const store = memoryStore()
    saveInvites(invites, store)
    const newer = { game: 'other', seats: [{ seat: 0, name: 'Cy', token: 't' }] }
    saveInvites(newer, store)
    expect(loadInvites(GAME, store)).toBeNull()
    expect(loadInvites('other', store)).toEqual(newer)
  })

  it('reads nothing when none was saved or another game was', () => {
    expect(loadInvites(GAME, memoryStore())).toBeNull()
    const store = memoryStore()
    saveInvites(invites, store)
    expect(loadInvites('another-game', store)).toBeNull()
  })

  it.each([
    ['text that is not JSON', 'not json'],
    ['JSON that is not an object', '[1,2]'],
    ['a record without seats', JSON.stringify({ game: GAME })],
    ['seats that are not a list', JSON.stringify({ game: GAME, seats: 'x' })],
    ['a seat without a token', JSON.stringify({ game: GAME, seats: [{ seat: 0, name: 'Ana' }] })],
    [
      'a seat number below zero',
      JSON.stringify({ game: GAME, seats: [{ seat: -1, name: 'Ana', token: 't' }] }),
    ],
    ['a token that is a number', JSON.stringify({ game: GAME, seats: [{ seat: 0, name: 'Ana', token: 5 }] })],
  ])('reads %s as nothing', (_name, stored) => {
    expect(loadInvites(GAME, memoryStore({ 'magic.invites': stored }))).toBeNull()
  })
})

describe('when the browser will not store anything', () => {
  it('reports a refused write instead of throwing', () => {
    expect(saveInvites(invites, throwing)).toBe(false)
  })

  it('reads nothing from a store that throws', () => {
    expect(loadInvites(GAME, throwing)).toBeNull()
  })

  it('treats a missing store like an empty one', () => {
    expect(saveInvites(invites, null)).toBe(false)
    expect(loadInvites(GAME, null)).toBeNull()
  })

  it('survives window.sessionStorage itself throwing, as it does with site data blocked', () => {
    vi.spyOn(window, 'sessionStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    expect(saveInvites(invites)).toBe(false)
    expect(loadInvites(GAME)).toBeNull()
  })

  it('uses the real sessionStorage by default', () => {
    expect(saveInvites(invites)).toBe(true)
    expect(loadInvites(GAME)).toEqual(invites)
    window.sessionStorage.clear()
  })
})

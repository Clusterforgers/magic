// Goal: a guest's name is kept in localStorage only as far as the browser allows: it comes back as saved, is cut to the length a name may have, and every failure (no storage, a store that throws on read or on write, an accessor that throws) reads as "nothing kept" and never as an error.
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { KeyValueStore } from '@/lib/invites'
import { loadGuestName, saveGuestName } from './guestName'

afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

const memory = (initial: Record<string, string> = {}): KeyValueStore => {
  const items = new Map(Object.entries(initial))
  return {
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
  }
}

const refusing: KeyValueStore = {
  getItem: () => {
    throw new DOMException('blocked', 'SecurityError')
  },
  setItem: () => {
    throw new DOMException('full', 'QuotaExceededError')
  },
}

describe('a store that works', () => {
  it('gives back what was saved', () => {
    const store = memory()
    expect(saveGuestName('Cy', store)).toBe(true)
    expect(loadGuestName(store)).toBe('Cy')
  })

  it('has nothing before anything is saved', () => {
    expect(loadGuestName(memory())).toBe('')
  })

  it.each([
    ['a long name', 'x'.repeat(40), 'x'.repeat(32)],
    ['a name of wide characters', '😀'.repeat(40), '😀'.repeat(32)],
    ['a name that fits', 'Ada Lovelace', 'Ada Lovelace'],
  ])('cuts %s to 32 characters on the way out', (_what, kept, shown) => {
    expect(loadGuestName(memory({ 'magic.guestName': kept }))).toBe(shown)
  })

  it('is the browser localStorage by default', () => {
    expect(saveGuestName('Cy')).toBe(true)
    expect(localStorage.getItem('magic.guestName')).toBe('Cy')
    expect(loadGuestName()).toBe('Cy')
  })
})

describe('a browser that keeps nothing', () => {
  it('has no store at all: nothing is kept and nothing throws', () => {
    expect(saveGuestName('Cy', null)).toBe(false)
    expect(loadGuestName(null)).toBe('')
  })

  it('has a store that throws on both: nothing is kept and nothing throws', () => {
    expect(saveGuestName('Cy', refusing)).toBe(false)
    expect(loadGuestName(refusing)).toBe('')
  })

  it('has localStorage itself throw when it is looked up', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('denied', 'SecurityError')
    })
    expect(saveGuestName('Cy')).toBe(false)
    expect(loadGuestName()).toBe('')
  })
})

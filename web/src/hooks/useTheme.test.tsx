// Goal: the theme is the person's choice or, by default, the system's: it follows a change of the system's preference while the page is open, survives a reload through storage and still works when storage or matchMedia are missing or throw.
import { act, renderHook } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createThemeStore, parseChoice, resolveTheme, THEME_KEY, useTheme } from './useTheme'
import type { ThemeChoice, ThemeDeps, ThemeStorage } from './useTheme'

class FakeMedia {
  matches: boolean
  listeners = new Set<() => void>()
  constructor(matches: boolean) {
    this.matches = matches
  }
  addEventListener(_type: 'change', listener: () => void): void {
    this.listeners.add(listener)
  }
  removeEventListener(_type: 'change', listener: () => void): void {
    this.listeners.delete(listener)
  }
  /** The system's preference changes. */
  set(matches: boolean): void {
    this.matches = matches
    for (const listener of [...this.listeners]) listener()
  }
}

class FakeStorage implements ThemeStorage {
  items = new Map<string, string>()
  getItem(key: string): string | null {
    return this.items.get(key) ?? null
  }
  setItem(key: string, value: string): void {
    this.items.set(key, value)
  }
  removeItem(key: string): void {
    this.items.delete(key)
  }
}

/** Storage that is there but refuses every use, as a browser with blocked site data does. */
const refusing: ThemeStorage = {
  getItem() {
    throw new DOMException('denied', 'SecurityError')
  },
  setItem() {
    throw new DOMException('denied', 'QuotaExceededError')
  },
  removeItem() {
    throw new DOMException('denied', 'SecurityError')
  },
}

/** Counts what is attached to it, which is what must be undone on unsubscribe. */
class FakeWindow extends EventTarget {
  attached = 0
  override addEventListener(...args: Parameters<EventTarget['addEventListener']>): void {
    this.attached += 1
    super.addEventListener(...args)
  }
  override removeEventListener(...args: Parameters<EventTarget['removeEventListener']>): void {
    this.attached -= 1
    super.removeEventListener(...args)
  }
  /** Another tab wrote `key`. */
  storage(key: string | null): void {
    this.dispatchEvent(Object.assign(new Event('storage'), { key }))
  }
}

function setup(options: { stored?: string; systemDark?: boolean } = {}) {
  const storage = new FakeStorage()
  if (options.stored !== undefined) storage.setItem(THEME_KEY, options.stored)
  const media = new FakeMedia(options.systemDark ?? false)
  const root = document.createElement('html')
  const events = new FakeWindow()
  const deps: ThemeDeps = { storage, media, root, events }
  return { storage, media, root, events, deps, store: createThemeStore(deps) }
}

describe('parseChoice and resolveTheme', () => {
  it.each<[unknown, ThemeChoice]>([
    ['light', 'light'],
    ['dark', 'dark'],
    ['system', 'system'],
    [null, 'system'],
    [undefined, 'system'],
    ['', 'system'],
    ['Dark', 'system'],
    ['auto', 'system'],
    [1, 'system'],
  ])('reads %j as %s', (value, expected) => {
    expect(parseChoice(value)).toBe(expected)
  })

  it.each<[ThemeChoice, boolean, 'light' | 'dark']>([
    ['light', false, 'light'],
    ['light', true, 'light'],
    ['dark', false, 'dark'],
    ['dark', true, 'dark'],
    ['system', false, 'light'],
    ['system', true, 'dark'],
  ])('shows %s while the system prefers dark: %s as %s', (choice, systemDark, expected) => {
    expect(resolveTheme(choice, systemDark)).toBe(expected)
  })
})

describe('the theme store', () => {
  it.each([
    [undefined, false, 'light', false],
    [undefined, true, 'dark', true],
    ['system', true, 'dark', true],
    ['light', true, 'light', false],
    ['dark', false, 'dark', true],
    ['nonsense', true, 'dark', true],
  ] as const)('starts from stored %s with the system dark %s: %s', (stored, systemDark, theme, dark) => {
    const { store, root } = setup({ stored, systemDark })
    expect(store.getTheme()).toBe(theme)
    expect(root.classList.contains('dark')).toBe(dark)
  })

  it('sets the class when it is made, so the page matches before anything is subscribed', () => {
    const { root } = setup({ stored: 'dark' })
    expect(root.classList.contains('dark')).toBe(true)
  })

  it('follows the system live while the choice is the system, and ignores it after an explicit choice', () => {
    const { store, media, root } = setup()
    const seen: string[] = []
    store.subscribe(() => seen.push(store.getTheme()))
    media.set(true)
    expect(root.classList.contains('dark')).toBe(true)
    media.set(false)
    expect(root.classList.contains('dark')).toBe(false)
    store.setChoice('light')
    media.set(true)
    expect(store.getTheme()).toBe('light')
    expect(root.classList.contains('dark')).toBe(false)
    expect(seen).toEqual(['dark', 'light', 'light', 'light'])
  })

  it('persists a choice, and forgets it for the system so a reload follows the system again', () => {
    const { store, storage } = setup()
    store.setChoice('dark')
    expect(storage.items.get(THEME_KEY)).toBe('dark')
    store.setChoice('light')
    expect(storage.items.get(THEME_KEY)).toBe('light')
    store.setChoice('system')
    expect(storage.items.has(THEME_KEY)).toBe(false)
  })

  it('reads the choice back from storage in a new store', () => {
    const first = setup()
    first.store.setChoice('dark')
    const second = createThemeStore({ ...first.deps, root: document.createElement('html') })
    expect(second.getChoice()).toBe('dark')
  })

  it('works when storage refuses every use: the choice lasts for the page', () => {
    const { deps, media } = setup()
    const store = createThemeStore({ ...deps, storage: refusing })
    expect(store.getChoice()).toBe('system')
    expect(() => store.setChoice('dark')).not.toThrow()
    expect(store.getChoice()).toBe('dark')
    expect(store.getTheme()).toBe('dark')
    media.set(false)
    expect(store.getTheme()).toBe('dark')
  })

  it('works with no storage, no matchMedia and nothing to listen to, as in a bare browser', () => {
    const root = document.createElement('html')
    const store = createThemeStore({ storage: null, media: null, root, events: null })
    const off = store.subscribe(() => undefined)
    expect(store.getTheme()).toBe('light')
    store.setChoice('dark')
    expect(root.classList.contains('dark')).toBe(true)
    expect(() => off()).not.toThrow()
  })

  it('follows another tab, and a storage that another tab cleared', () => {
    const { store, storage, events, root } = setup()
    store.subscribe(() => undefined)
    storage.setItem(THEME_KEY, 'dark')
    events.storage(THEME_KEY)
    expect(store.getChoice()).toBe('dark')
    expect(root.classList.contains('dark')).toBe(true)
    events.storage('something-else')
    storage.items.clear()
    expect(store.getChoice()).toBe('dark')
    events.storage(null)
    expect(store.getChoice()).toBe('system')
    expect(root.classList.contains('dark')).toBe(false)
  })

  it('listens only while something is subscribed', () => {
    const { store, media, events } = setup()
    const first = store.subscribe(() => undefined)
    const second = store.subscribe(() => undefined)
    expect([media.listeners.size, events.attached]).toEqual([1, 1])
    first()
    expect([media.listeners.size, events.attached]).toEqual([1, 1])
    second()
    expect([media.listeners.size, events.attached]).toEqual([0, 0])
  })

  it('catches up with a preference that moved while nothing listened', () => {
    const { store, media, root } = setup()
    media.matches = true
    expect(root.classList.contains('dark')).toBe(false)
    store.subscribe(() => undefined)
    expect(root.classList.contains('dark')).toBe(true)
  })
})

describe('useTheme', () => {
  it('gives the choice and the theme, and re-renders for a choice and for the system', () => {
    const { store, media } = setup({ systemDark: false })
    const { result } = renderHook(() => useTheme(store))
    expect(result.current).toMatchObject({ choice: 'system', theme: 'light' })
    act(() => media.set(true))
    expect(result.current).toMatchObject({ choice: 'system', theme: 'dark' })
    act(() => result.current.setChoice('light'))
    expect(result.current).toMatchObject({ choice: 'light', theme: 'light' })
  })

  it('stops listening when the component goes', () => {
    const { store, media } = setup()
    const { unmount } = renderHook(() => useTheme(store))
    expect(media.listeners.size).toBe(1)
    unmount()
    expect(media.listeners.size).toBe(0)
  })
})

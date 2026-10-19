import { useSyncExternalStore } from 'react'

/** What a person picks: a theme, or whatever their system asks for. */
export type ThemeChoice = 'light' | 'dark' | 'system'

/** The theme on screen. */
export type Theme = 'light' | 'dark'

/** The `localStorage` key of the choice; the script in `index.html` reads the same one before the first paint. */
export const THEME_KEY = 'theme'

const DARK_QUERY = '(prefers-color-scheme: dark)'

/** The choice a stored value stands for; anything unknown, or nothing, means the system's. */
export function parseChoice(value: unknown): ThemeChoice {
  return value === 'light' || value === 'dark' ? value : 'system'
}

/** The theme a choice shows while the system prefers dark or not. */
export function resolveTheme(choice: ThemeChoice, systemDark: boolean): Theme {
  if (choice === 'system') return systemDark ? 'dark' : 'light'
  return choice
}

/** The part of `Storage` the theme needs. */
export type ThemeStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

/** The part of a `MediaQueryList` the theme needs. */
export interface ThemeMedia {
  matches: boolean
  addEventListener(type: 'change', listener: () => void): void
  removeEventListener(type: 'change', listener: () => void): void
}

/** The part of `window` that delivers the `storage` events of other tabs. */
export interface ThemeEvents {
  addEventListener(type: 'storage', listener: (event: Event) => void): void
  removeEventListener(type: 'storage', listener: (event: Event) => void): void
}

/** What a theme store reads and writes; every part may be missing, and the theme then follows the system for the session. */
export interface ThemeDeps {
  storage: ThemeStorage | null
  /** The `prefers-color-scheme: dark` query, or null where the browser has no `matchMedia`. */
  media: ThemeMedia | null
  /** The element that carries the `dark` class. */
  root: HTMLElement
  /** Where other tabs' changes arrive. */
  events: ThemeEvents | null
}

/** The theme of the page: the person's choice, the system's preference, and the class on the root element. */
export interface ThemeStore {
  getChoice(): ThemeChoice
  getTheme(): Theme
  setChoice(choice: ThemeChoice): void
  subscribe(listener: () => void): () => void
}

// Browsers can refuse storage altogether (blocked site data, a private window), by returning nothing or by throwing.
function guarded<T>(read: () => T, fallback: T): T {
  try {
    return read()
  } catch {
    return fallback
  }
}

/**
 * The browser's own parts. Each is looked up in a guard, because `localStorage` itself can throw on access and jsdom
 * has no `matchMedia`.
 */
export function browserThemeDeps(): ThemeDeps {
  return {
    storage: guarded(() => window.localStorage, null),
    media: guarded(
      () => (typeof window.matchMedia === 'function' ? window.matchMedia(DARK_QUERY) : null),
      null,
    ),
    root: document.documentElement,
    events: window,
  }
}

/**
 * A store for the theme. The choice is kept in `storage` when it can be (the system's choice by removing the key), the
 * `dark` class is on `root` whenever the theme is dark, and a change of the system's preference or of another tab's
 * choice is followed while anything is subscribed. Without storage the choice lasts until the page closes.
 */
export function createThemeStore(deps: ThemeDeps = browserThemeDeps()): ThemeStore {
  const { storage, media, root, events } = deps
  const listeners = new Set<() => void>()
  let choice = parseChoice(guarded(() => storage?.getItem(THEME_KEY), null))
  let theme = resolveTheme(choice, media?.matches ?? false)

  const apply = (): void => {
    theme = resolveTheme(choice, media?.matches ?? false)
    root.classList.toggle('dark', theme === 'dark')
  }
  const refresh = (): void => {
    apply()
    for (const listener of listeners) listener()
  }
  const onStorage = (event: Event): void => {
    const { key } = event as StorageEvent
    // A null key means the other tab cleared everything.
    if (key !== THEME_KEY && key !== null) return
    choice = parseChoice(guarded(() => storage?.getItem(THEME_KEY), null))
    refresh()
  }
  apply()

  return {
    getChoice: () => choice,
    getTheme: () => theme,
    setChoice(next) {
      choice = next
      guarded(
        () => (next === 'system' ? storage?.removeItem(THEME_KEY) : storage?.setItem(THEME_KEY, next)),
        undefined,
      )
      refresh()
    },
    subscribe(listener) {
      if (listeners.size === 0) {
        media?.addEventListener('change', refresh)
        events?.addEventListener('storage', onStorage)
        // The preference may have moved while nothing listened.
        apply()
      }
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
        if (listeners.size === 0) {
          media?.removeEventListener('change', refresh)
          events?.removeEventListener('storage', onStorage)
        }
      }
    },
  }
}

let shared: ThemeStore | null = null

/** The app's one theme store, made on first use so importing this file touches nothing. */
export function appTheme(): ThemeStore {
  shared ??= createThemeStore()
  return shared
}

/** The choice, the theme on screen and a way to change the choice; follows the system and other tabs. */
export function useTheme(store: ThemeStore = appTheme()): {
  choice: ThemeChoice
  theme: Theme
  setChoice(choice: ThemeChoice): void
} {
  const choice = useSyncExternalStore(store.subscribe, store.getChoice, () => 'system' as const)
  const theme = useSyncExternalStore(store.subscribe, store.getTheme, () => 'light' as const)
  return { choice, theme, setChoice: store.setChoice }
}

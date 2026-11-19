/**
 * Loading the pictures of a game's cards ahead of play (`DeckPictures`, [web.md](../../../../docs/web.md), Card images):
 * in the sizes the table draws, a few at a time while the page is idle, never before the first render or ahead of a
 * picture the table asks for. The list is the server's, which names another seat's hidden cards never.
 */
import type { PictureCard } from '@/protocol/generated'
import { cardImageUrl } from './images'
import type { ImageSize } from './images'

/** Pictures loaded ahead at once: few, so a picture the table asks for never waits behind them. */
export const PREFETCH_AT_ONCE = 2

/** The sizes the table draws by width, smallest first: what a prefetch may load. */
const TABLE_SIZES: readonly ImageSize[] = ['small', 'normal', 'large']

/** The size loaded ahead before the table has drawn any picture. */
export const FIRST_SIZE: ImageSize = 'normal'

/**
 * The URLs to load ahead: every face with a picture of each card in each size, one size after the other, so the size
 * the table drew first is complete first. Each URL once.
 */
export function prefetchUrls(cards: readonly PictureCard[], sizes: readonly ImageSize[]): string[] {
  const urls = new Set<string>()
  for (const size of sizes) {
    for (const card of cards) {
      const faces = Math.max(1, Math.trunc(card.faces))
      for (let face = 0; face < faces; face += 1) {
        urls.add(cardImageUrl({ kind: 'card', identity: card.identity }, face, size))
      }
    }
  }
  return [...urls]
}

/** What the browser says of the connection (Network Information API, where there is one). */
export interface ConnectionHint {
  saveData?: boolean
  effectiveType?: string
}

/**
 * Whether pictures may be loaded ahead: not when the viewer chose text faces, asked to save data or is on a 2G
 * connection, where every byte is one the table may wait for.
 */
export function mayPrefetch(choice: 'on' | 'text', connection: ConnectionHint | undefined): boolean {
  if (choice !== 'on') return false
  if (connection?.saveData === true) return false
  return connection?.effectiveType !== 'slow-2g' && connection?.effectiveType !== '2g'
}

// The sizes the table drew by width this session, in the order first drawn; the prefetch follows them.
const drawn: ImageSize[] = []
const listeners = new Set<() => void>()

/**
 * Notes a size the table drew a card picture in by its width (`CardFace`, while it renders), so a prefetch loads that
 * size. The listeners hear of it after the render.
 */
export function noteDrawnSize(size: ImageSize): void {
  if (!TABLE_SIZES.includes(size) || drawn.includes(size)) return
  drawn.push(size)
  queueMicrotask(() => {
    for (const listener of listeners) listener()
  })
}

/** The sizes the table drew so far, else the first guess. */
export function drawnSizes(): ImageSize[] {
  return drawn.length === 0 ? [FIRST_SIZE] : [...drawn]
}

/** Runs `listener` whenever the table draws a new size; returns the unsubscribe. */
export function onDrawnSize(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Forgets the sizes drawn (tests). */
export function resetDrawnSizes(): void {
  drawn.length = 0
}

/** How a prefetcher loads one URL and waits for an idle moment; injected so tests run without a page. */
export interface PrefetchDeps {
  /** Loads `url` at low priority; resolves either way. */
  load(url: string): Promise<boolean>
  /** Runs `task` when the page is idle. Returns a cancel. */
  idle(task: () => void): () => void
  /** Told of each URL loaded. */
  loaded?(url: string): void
}

/** A queue of URLs to load ahead. */
export interface Prefetcher {
  /** Adds URLs not asked for before, in order. */
  add(urls: readonly string[]): void
  /** Drops what still waits; loads under way finish. */
  stop(): void
  /** URLs still waiting. */
  readonly waiting: number
}

/** Builds a prefetcher that loads at most `limit` URLs at once, each started at an idle moment. */
export function createPrefetcher(deps: PrefetchDeps, limit: number = PREFETCH_AT_ONCE): Prefetcher {
  const asked = new Set<string>()
  const queue: string[] = []
  let active = 0
  let stopped = false
  const cancels = new Set<() => void>()

  function pump(): void {
    while (!stopped && active < limit && queue.length > 0) {
      const url = queue.shift() as string
      active += 1
      const cancel = deps.idle(() => {
        cancels.delete(cancel)
        void deps.load(url).then((ok) => {
          active -= 1
          if (ok) deps.loaded?.(url)
          pump()
        })
      })
      cancels.add(cancel)
    }
  }

  return {
    add(urls) {
      if (stopped) return
      for (const url of urls) {
        if (asked.has(url)) continue
        asked.add(url)
        queue.push(url)
      }
      pump()
    },
    stop() {
      stopped = true
      queue.length = 0
      for (const cancel of cancels) cancel()
      cancels.clear()
    },
    get waiting() {
      return queue.length
    },
  }
}

/** Loads a picture into the browser's cache at low priority, as an `<img>` would ask for it. */
export function loadPicture(url: string): Promise<boolean> {
  return new Promise((resolve) => {
    const img = new Image()
    img.decoding = 'async'
    img.fetchPriority = 'low'
    img.onload = () => resolve(true)
    img.onerror = () => resolve(false)
    img.src = url
  })
}

/** Runs `task` when the page is idle (within 2 s), else after a short pause where the browser cannot tell. */
export function whenIdle(task: () => void): () => void {
  if (typeof window.requestIdleCallback === 'function') {
    const handle = window.requestIdleCallback(task, { timeout: 2000 })
    return () => window.cancelIdleCallback(handle)
  }
  const handle = window.setTimeout(task, 200)
  return () => window.clearTimeout(handle)
}

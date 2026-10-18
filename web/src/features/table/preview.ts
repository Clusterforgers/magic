/** The big card preview: which card is shown, where it docks, and the store the cards talk to. */
import { createContext, useContext, useSyncExternalStore } from 'react'
import type { ObjectId } from '@/protocol/generated'

/** The box of the card that asked for a preview, in viewport pixels. */
export interface Anchor {
  left: number
  top: number
  width: number
  height: number
}

/** The card on show. A pinned preview (from a click or tap) stays until dismissed; hover and focus come and go. */
export interface PreviewTarget {
  id: ObjectId
  anchor: Anchor
  pinned: boolean
}

/** The edge of the screen the preview sits at, always on the side away from its card. */
export type Dock = 'left' | 'right' | 'top' | 'bottom'

/** The window the preview must fit in. */
export interface Viewport {
  width: number
  height: number
}

/** From this width the preview docks left or right; narrower screens dock it top or bottom. */
export const SIDE_DOCK_FROM = 640

/** Docks away from the card: opposite side horizontally on wide screens, opposite half vertically on narrow ones. */
export function dockOf(anchor: Anchor, viewport: Viewport): Dock {
  const x = anchor.left + anchor.width / 2
  const y = anchor.top + anchor.height / 2
  if (viewport.width >= SIDE_DOCK_FROM) return x > viewport.width / 2 ? 'left' : 'right'
  return y > viewport.height / 2 ? 'top' : 'bottom'
}

/** The preview state; cards call it, `CardPreview` renders it. */
export interface PreviewStore {
  get(): PreviewTarget | null
  subscribe(listener: () => void): () => void
  /** Hover or keyboard focus on a card. */
  show(id: ObjectId, anchor: Anchor): void
  /** The pointer left or focus moved off a card; a pinned preview stays. */
  hide(id: ObjectId): void
  /** A click or tap: pins the card, or unpins it when it is the pinned one already. */
  pin(id: ObjectId, anchor: Anchor): void
  /** Closes whatever is shown. */
  dismiss(): void
}

/** A store with nothing shown. */
export function createPreviewStore(): PreviewStore {
  let target: PreviewTarget | null = null
  const listeners = new Set<() => void>()
  const set = (next: PreviewTarget | null): void => {
    if (target === next) return
    target = next
    for (const listener of listeners) listener()
  }
  return {
    get: () => target,
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    show(id, anchor) {
      if (target?.pinned === true && target.id === id) return
      set({ id, anchor, pinned: false })
    },
    hide(id) {
      if (target !== null && !target.pinned && target.id === id) set(null)
    },
    pin(id, anchor) {
      set(target?.pinned === true && target.id === id ? null : { id, anchor, pinned: true })
    },
    dismiss: () => set(null),
  }
}

/** Cards outside a table share this one, which nothing renders. */
export const PreviewContext = createContext<PreviewStore>(createPreviewStore())

/** The preview store of the table around the caller. */
export function usePreviewStore(): PreviewStore {
  return useContext(PreviewContext)
}

/** The card on show now, or null. */
export function usePreviewTarget(): PreviewTarget | null {
  const store = usePreviewStore()
  return useSyncExternalStore(store.subscribe, store.get)
}

/** The box of an element in viewport pixels. */
export function anchorOf(element: Element): Anchor {
  const { left, top, width, height } = element.getBoundingClientRect()
  return { left, top, width, height }
}

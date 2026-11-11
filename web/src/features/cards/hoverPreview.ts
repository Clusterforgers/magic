/** The one large card preview of the pages outside the table: which card is shown, where, and whether it is pinned. */
import { useSyncExternalStore } from 'react'
import type { HoverFit } from './hoverFit'

/** The card on show: the key of the element that asked, where the panel stands, and whether a click pinned it. */
export interface HoverTarget {
  key: string
  at: HoverFit
  pinned: boolean
}

/** The preview state; `HoverPicture` elements call it and the one whose key is shown draws the panel. */
export interface HoverPreviewStore {
  get(): HoverTarget | null
  subscribe(listener: () => void): () => void
  /** A mouse hover or keyboard focus; ignored while a card is pinned. */
  show(key: string, at: HoverFit): void
  /** The pointer or focus left; a pinned preview stays. */
  hide(key: string): void
  /** A click: pins the card, or unpins it when it is the pinned one already. */
  pin(key: string, at: HoverFit): void
  /** Closes whatever is shown. */
  dismiss(): void
}

/** A store with nothing shown; the rules match the table's preview store. */
export function createHoverPreviewStore(): HoverPreviewStore {
  let target: HoverTarget | null = null
  const listeners = new Set<() => void>()
  const set = (next: HoverTarget | null): void => {
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
    show(key, at) {
      if (target?.pinned === true) return
      set({ key, at, pinned: false })
    },
    hide(key) {
      if (target !== null && !target.pinned && target.key === key) set(null)
    },
    pin(key, at) {
      set(target?.pinned === true && target.key === key ? null : { key, at, pinned: true })
    },
    dismiss: () => set(null),
  }
}

/** The store every page outside the table shares. */
export const hoverPreview = createHoverPreviewStore()

/** The card on show now, or null. */
export function useHoverTarget(): HoverTarget | null {
  return useSyncExternalStore(hoverPreview.subscribe, hoverPreview.get)
}

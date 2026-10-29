/** Which card's manual menu is open and where; the cards talk to this store and `ManualMenuHost` draws the menu. */
import { createContext, useContext, useSyncExternalStore } from 'react'
import type { ObjectId } from '@/protocol/generated'

/** The menu on show: the card, the point it opens at (viewport pixels) and the element that asked, which gets the focus back. */
export interface MenuTarget {
  id: ObjectId
  x: number
  y: number
  opener: HTMLElement | null
}

/** The open menu, if any. */
export interface MenuStore {
  get(): MenuTarget | null
  subscribe(listener: () => void): () => void
  open(target: MenuTarget): void
  close(): void
}

/** A store with no menu open. */
export function createMenuStore(): MenuStore {
  let target: MenuTarget | null = null
  const listeners = new Set<() => void>()
  const set = (next: MenuTarget | null): void => {
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
    // A second request for the same card at the same place (a key and the contextmenu event it causes) changes nothing.
    open(next) {
      if (target?.id === next.id && target.x === next.x && target.y === next.y) return
      set(next)
    },
    close: () => {
      if (target !== null) set(null)
    },
  }
}

/** The store of the table around a card; null outside a table, where cards have no menu. */
export const ManualMenuContext = createContext<MenuStore | null>(null)

/** The menu store of the table around the caller, or null. */
export function useMenuStore(): MenuStore | null {
  return useContext(ManualMenuContext)
}

const NEVER = (): (() => void) => () => undefined
const NOTHING = (): null => null

/** The menu on show now, or null. */
export function useMenuTarget(): MenuTarget | null {
  const store = useMenuStore()
  return useSyncExternalStore(store?.subscribe ?? NEVER, store?.get ?? NOTHING)
}

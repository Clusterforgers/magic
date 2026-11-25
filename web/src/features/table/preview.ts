/** The big card preview: which card is shown, where it docks, and the store the cards talk to. */
import { createContext, useContext, useSyncExternalStore } from 'react'
import type { CustomCardView, ObjectId, ObjectView } from '@/protocol/generated'

/** The box of the card that asked for a preview, in viewport pixels. */
export interface Anchor {
  left: number
  top: number
  width: number
  height: number
}

/** The card on show. A pinned preview (from a click or tap) stays until dismissed; hover and focus come and go beside no pin. */
export interface PreviewTarget {
  id: ObjectId
  anchor: Anchor
  pinned: boolean
  /** The card as every seat was shown it, for one the view may hold no longer (a revealed card that moved on); drawn instead of the view's. */
  shown?: ObjectView
  /** A custom card of a Delves fight (a boss's innate, phase or relic, a hero's relic), drawn as its framed card. */
  custom?: CustomShown
}

/** A custom card the preview draws in its frame, with what it is to the fight. */
export interface CustomShown {
  card: CustomCardView
  /** What it is now: "Innate · Level III", "Phase 1 · in force", "Boss relic". */
  kicker?: string
  /** Where it is: "In Cy's command zone". */
  place?: string
  /** A boss's phases in order, this one among them, so the preview can step to another. */
  phases?: readonly CustomShown[]
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
  /** Hover or keyboard focus on a card; ignored while a card is pinned, so a pin stays until it is closed. */
  show(id: ObjectId, anchor: Anchor, shown?: ObjectView, custom?: CustomShown): void
  /** The pointer left or focus moved off a card; a pinned preview stays. */
  hide(id: ObjectId): void
  /** A click or tap: pins the card, or unpins it when it is the pinned one already. */
  pin(id: ObjectId, anchor: Anchor, shown?: ObjectView, custom?: CustomShown): void
  /** Closes whatever is shown. */
  dismiss(): void
}

/** Whether a lesson holds the hover preview closed (a button is lit), and who hears when that changes. */
const hold = { on: false, listeners: new Set<() => void>() }

/** Holds the preview closed while `on` (a lesson step that is no look at a card), so it never covers what the coach lights. */
export function holdPreview(on: boolean): void {
  if (hold.on === on) return
  hold.on = on
  for (const listener of hold.listeners) listener()
}

/** True while the hover preview is held closed. */
export function previewHeld(): boolean {
  return hold.on
}

/** The optional parts of a target: a card as shown, a custom card. */
function extra(shown?: ObjectView, custom?: CustomShown): Pick<PreviewTarget, 'shown' | 'custom'> {
  return { ...(shown === undefined ? {} : { shown }), ...(custom === undefined ? {} : { custom }) }
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
  hold.listeners.add(() => {
    if (hold.on && target !== null) set(null)
  })
  return {
    get: () => target,
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    show(id, anchor, shown, custom) {
      if (target?.pinned === true || hold.on) return
      set({ id, anchor, pinned: false, ...extra(shown, custom) })
    },
    hide(id) {
      if (target !== null && !target.pinned && target.id === id) set(null)
    },
    pin(id, anchor, shown, custom) {
      if (hold.on) return
      set(
        target?.pinned === true && target.id === id
          ? null
          : { id, anchor, pinned: true, ...extra(shown, custom) },
      )
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

import { useEffect, useMemo, useRef } from 'react'
import type { KeyboardEvent, MouseEvent, PointerEvent } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId } from '@/protocol/generated'
import { clausesOf, opensClauses, showClauses } from './clauses'
import { useMenuStore } from './menuStore'
import type { MenuStore } from './menuStore'

/** The events that open the menu of one card; spread them on its element. Empty where there is no menu to open. */
export interface ManualMenuHandlers {
  onContextMenu?(event: MouseEvent<HTMLElement>): void
  onKeyDown?(event: KeyboardEvent<HTMLElement>): void
  onPointerDown?(event: PointerEvent<HTMLElement>): void
  onPointerMove?(event: PointerEvent<HTMLElement>): void
  onPointerUp?(): void
  onPointerCancel?(): void
  onClickCapture?(event: MouseEvent<HTMLElement>): void
}

/** How long a finger rests on a card before the menu opens, in milliseconds. */
export const LONG_PRESS_MS = 500
/** How far a finger may drift and still be resting, in pixels. */
const DRIFT_PX = 10
/** How long after a long press the click that follows it is swallowed. */
const SWALLOW_MS = 800

const NONE: ManualMenuHandlers = {}

/** Whether a key press asks for the menu: `m`, the Menu key or Shift+F10, with no other modifier. */
export function opensMenu(
  event: Pick<KeyboardEvent, 'key' | 'shiftKey' | 'ctrlKey' | 'altKey' | 'metaKey'>,
): boolean {
  if (event.ctrlKey || event.altKey || event.metaKey) return false
  if (event.key === 'ContextMenu') return true
  if (event.key === 'F10') return event.shiftKey
  return event.key.toLowerCase() === 'm' && !event.shiftKey
}

function openAt(store: MenuStore, id: ObjectId, element: HTMLElement, x: number, y: number): void {
  store.open({ id, x, y, opener: element })
}

/** Opens the menu below the element, for a key press. */
function openBelow(store: MenuStore, id: ObjectId, element: HTMLElement): void {
  const { left, bottom } = element.getBoundingClientRect()
  openAt(store, id, element, left, bottom)
}

/**
 * What opens the manual menu of a card: a right click, a long press of a finger, and the keyboard (`m`, the Menu key,
 * Shift+F10) while the card is focused. A seat that only watches has none, and the browser's own menu stays. A card with
 * clauses left to the table also lists them on `h`, for a seat that plays and one that watches.
 */
export function useManualMenu(id: ObjectId): ManualMenuHandlers {
  const store = useMenuStore()
  const plays = usePlay((state) => state.seat !== null && state.view?.spectator !== true)
  const clauses = usePlay((state) => {
    const object = state.objectsById[id]
    return object !== undefined && clausesOf(object).length > 0
  })
  const timer = useRef<number | undefined>(undefined)
  const start = useRef({ x: 0, y: 0 })
  const swallow = useRef(false)
  useEffect(() => () => window.clearTimeout(timer.current), [])
  return useMemo(() => {
    if (store === null) return NONE
    // The list of the clauses is for everybody who sees the card; the focused card is its opener.
    const lists = (event: KeyboardEvent<HTMLElement>): boolean => {
      if (!clauses || event.target !== event.currentTarget || !opensClauses(event)) return false
      event.preventDefault()
      showClauses(store, id, event.currentTarget, event.currentTarget)
      return true
    }
    if (!plays) return clauses ? { onKeyDown: lists } : NONE
    const cancel = (): void => window.clearTimeout(timer.current)
    return {
      onContextMenu(event) {
        event.preventDefault()
        // The Menu key and Shift+F10 report no position; the card's corner stands for it.
        if (event.clientX === 0 && event.clientY === 0) openBelow(store, id, event.currentTarget)
        else openAt(store, id, event.currentTarget, event.clientX, event.clientY)
      },
      onKeyDown(event) {
        if (lists(event) || event.target !== event.currentTarget || !opensMenu(event)) return
        event.preventDefault()
        openBelow(store, id, event.currentTarget)
      },
      onPointerDown(event) {
        if (event.pointerType !== 'touch') return
        const element = event.currentTarget
        start.current = { x: event.clientX, y: event.clientY }
        cancel()
        timer.current = window.setTimeout(() => {
          swallow.current = true
          window.setTimeout(() => {
            swallow.current = false
          }, SWALLOW_MS)
          openAt(store, id, element, start.current.x, start.current.y)
        }, LONG_PRESS_MS)
      },
      onPointerMove(event) {
        if (Math.hypot(event.clientX - start.current.x, event.clientY - start.current.y) > DRIFT_PX) cancel()
      },
      onPointerUp: cancel,
      onPointerCancel: cancel,
      // The click that ends a long press is not a tap on the card.
      onClickCapture(event) {
        if (!swallow.current) return
        swallow.current = false
        event.preventDefault()
        event.stopPropagation()
      },
    }
  }, [store, plays, clauses, id])
}

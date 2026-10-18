import { useCallback, useState } from 'react'
import type { FocusEvent, KeyboardEvent } from 'react'
import type { ObjectId } from '@/protocol/generated'

/** The object id a focused element belongs to, or null. */
export function objectIdOf(element: Element): ObjectId | null {
  const raw = element.closest('[data-object-id]')?.getAttribute('data-object-id')
  const id = raw === null || raw === undefined ? Number.NaN : Number(raw)
  return Number.isInteger(id) ? id : null
}

/** Where an arrow key moves among `count` items from `index`, or null for a key that does not move. */
export function rovingTarget(key: string, index: number, count: number): number | null {
  if (count === 0) return null
  switch (key) {
    case 'ArrowRight':
    case 'ArrowDown':
      return Math.min(index + 1, count - 1)
    case 'ArrowLeft':
    case 'ArrowUp':
      return Math.max(index - 1, 0)
    case 'Home':
      return 0
    case 'End':
      return count - 1
    default:
      return null
  }
}

/** What a list of cards spreads on its element and which card is its one tab stop. */
export interface Roving {
  /** The card that is in the tab order: the last one focused, else the first. */
  current: ObjectId | null
  onFocus(event: FocusEvent<HTMLElement>): void
  onKeyDown(event: KeyboardEvent<HTMLElement>): void
}

/**
 * Roving focus for a row of cards: Tab enters the row once, the arrow keys, Home and End move within it. A row of 40
 * permanents is then one stop, not forty. Cards are found by `data-object-id`, so the row needs no refs.
 */
export function useRoving(ids: readonly ObjectId[]): Roving {
  const [last, setLast] = useState<ObjectId | null>(null)
  const current = last !== null && ids.includes(last) ? last : (ids[0] ?? null)
  const onFocus = useCallback((event: FocusEvent<HTMLElement>) => {
    const id = objectIdOf(event.target)
    if (id !== null) setLast(id)
  }, [])
  const onKeyDown = useCallback((event: KeyboardEvent<HTMLElement>) => {
    if (event.altKey || event.ctrlKey || event.metaKey) return
    const cards = [...event.currentTarget.querySelectorAll<HTMLElement>('button[data-object-id]')]
    const index = cards.findIndex((card) => card === document.activeElement)
    const to = rovingTarget(event.key, Math.max(index, 0), cards.length)
    if (index < 0 || to === null) return
    event.preventDefault()
    cards[to]?.focus()
  }, [])
  return { current, onFocus, onKeyDown }
}

/** Which cards the table draws, so a prompt asks for the others in its own pictures and lets the table take these. */
import { useLayoutEffect, useState } from 'react'
import type { ObjectId } from '@/protocol/generated'

const NONE: ReadonlySet<ObjectId> = new Set()

/** Whether a board or the hand draws the card `id` (a card of the table carries `data-size`; a chip does not). */
export function drawnOnTable(id: ObjectId, root: ParentNode = document): boolean {
  return (
    root.querySelector(
      `:is([data-area="opponents"], [data-area="own"], [data-area="hand"]) button[data-object-id="${id}"][data-size]`,
    ) !== null
  )
}

const same = (a: ReadonlySet<ObjectId>, b: ReadonlySet<ObjectId>): boolean =>
  a.size === b.size && [...a].every((id) => b.has(id))

/**
 * The cards of `ids` the table draws, read after every render (the boards lay out a frame after a prompt arrives, and
 * a focused board changes which seats are drawn). The same set until it changes.
 */
export function useDrawnCards(ids: readonly ObjectId[]): ReadonlySet<ObjectId> {
  const [drawn, setDrawn] = useState(NONE)
  useLayoutEffect(() => {
    const now = new Set(ids.filter((id) => drawnOnTable(id)))
    setDrawn((was) => (same(was, now) ? was : now))
  })
  return drawn
}

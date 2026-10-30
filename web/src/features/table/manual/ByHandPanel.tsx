/** The list of a card's clauses that the game leaves to the table, in a small popup (a sheet on a phone) at its mark. */
import { useEffect, useId, useRef } from 'react'
import type { CSSProperties, KeyboardEvent } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { BY_HAND } from '@/lib/byHand'
import { ByHandList } from './ByHand'
import { clausesOf } from './clauses'
import { useMenuStore } from './menuStore'
import type { MenuTarget } from './menuStore'
import { useCloseOutside, usePlaced, useReturnFocus } from './popup'

/** The popup of one card, open at `target`: a dialog that takes the focus; Escape, Tab and Close give it back to the card. */
export function ByHandPanel({ target }: { target: MenuTarget }) {
  const store = useMenuStore()
  const panel = useRef<HTMLDivElement>(null)
  const listId = useId()
  const object = usePlay((state) => state.objectsById[target.id])
  const clauses = object === undefined ? [] : clausesOf(object)
  const gone = clauses.length === 0
  useReturnFocus(target)
  useCloseOutside(panel, store)
  usePlaced(panel, target)
  // The card left the table, or its clauses went, while the popup was open.
  useEffect(() => {
    if (gone) store?.close()
  }, [gone, store])
  useEffect(() => {
    panel.current?.focus({ preventScroll: true })
  }, [])
  if (object === undefined || gone) return null
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    event.stopPropagation()
    // Tab steps from the dialog to Close, and from Close (or back from the dialog) out: to the card.
    const leaves = event.key === 'Tab' && (event.shiftKey || event.target !== event.currentTarget)
    if (event.key !== 'Escape' && !leaves) return
    event.preventDefault()
    store?.close()
  }
  return (
    <div
      ref={panel}
      role="dialog"
      tabIndex={-1}
      aria-label={`${object.name}: rules ${BY_HAND}`}
      aria-describedby={listId}
      data-slot="by-hand-panel"
      data-card={target.id}
      onKeyDown={onKeyDown}
      style={{ '--mx': `${target.x}px`, '--my': `${target.y}px` } as CSSProperties}
      className="fixed z-[70] grid max-h-[min(24rem,calc(100dvh-1rem))] w-72 max-w-[calc(100vw-1rem)] gap-2 overflow-y-auto rounded-lg border bg-popover p-3 text-popover-foreground shadow-e2 outline-none max-sm:inset-x-2 max-sm:bottom-2 max-sm:w-auto max-sm:max-h-[72dvh] sm:top-(--my) sm:left-(--mx)"
    >
      <div className="grid gap-1">
        <p className="truncate text-sm font-medium">{object.name}</p>
        <p className="text-xs font-medium">By hand</p>
        <p className="text-xs text-muted-foreground">The game does not run these. The table applies them.</p>
      </div>
      <ByHandList id={listId} clauses={clauses} className="rounded-md bg-muted px-2 py-2 text-xs" />
      <button
        type="button"
        onClick={() => store?.close()}
        className="justify-self-end rounded-md border px-3 py-1 text-sm outline-primary hover:bg-accent focus-visible:outline-2 pointer-coarse:min-h-11"
      >
        Close
      </button>
    </div>
  )
}

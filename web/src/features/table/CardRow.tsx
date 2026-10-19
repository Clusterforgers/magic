import { useCallback, useLayoutEffect, useMemo, useRef } from 'react'
import type { CSSProperties, KeyboardEvent } from 'react'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { CardStack } from './CardStack'
import { GhostCard } from './fx/GhostCard'
import { withGhosts } from './fx/ghosts'
import { NO_GHOSTS } from './fx/model'
import type { Ghost } from './fx/model'
import type { RowEntry } from './rowStacks'
import { objectIdOf, useRoving } from './useRoving'

interface CardRowProps {
  /** What the row draws, in order. With `stacks`, the first member of a closed stack and every member of an open one. */
  ids: readonly ObjectId[]
  /**
   * The stacks of a battlefield row (`layoutSeat`): a closed one is drawn as a `CardStack` with a "×N" chip, an open
   * one as its members, the first with a chip that closes it unless a prompt holds it open. Without them every id is a card.
   */
  stacks?: readonly RowEntry[]
  /** Opens a closed stack and closes an open one, named by any of its members. */
  onToggleStack?: (id: ObjectId) => void
  /** Permanents that just died, drawn in their old places while their effect plays; decoration, never in `ids`. */
  ghosts?: readonly Ghost[]
  /** Names the list for a screen reader, such as "Alice's creatures". */
  label: string
  /** Which parts of a face are drawn; the box too, unless `box` fits it. */
  size: CardSize
  /**
   * The box of every card in the row in px, from the fit of its zone (`layoutSeat`): written as `--card-w` and
   * `--card-h` on the row, with the gaps the fit engine counts. Without it each card takes the box of its `size`.
   */
  box?: CardBox
  /**
   * The space between neighbours in px, negative to overlap them (a hand that is wider than its dock). A hovered or
   * focused card comes to the front. Only with `box`.
   */
  gap?: number
  /** One line that scrolls sideways instead of wrapping onto more lines. */
  single?: boolean
  className?: string
}

/** A card's box in px. */
export interface CardBox {
  width: number
  height: number
}

/** Every member of a stack of two or more, by id, to the entry that holds it. */
function entriesById(stacks: readonly RowEntry[] | undefined): ReadonlyMap<ObjectId, RowEntry> {
  const byId = new Map<ObjectId, RowEntry>()
  for (const entry of stacks ?? []) {
    if (entry.ids.length > 1) for (const id of entry.ids) byId.set(id, entry)
  }
  return byId
}

/**
 * A list of cards with one tab stop: Tab enters it, the arrow keys, Home and End move along it. Stacks open and close
 * in place; the card that was focused (or the stack it was in) keeps the focus, and Escape closes the stack the focus
 * is in.
 */
export function CardRow({
  ids,
  stacks,
  onToggleStack,
  ghosts = NO_GHOSTS,
  label,
  size,
  box,
  gap,
  single = false,
  className,
}: CardRowProps) {
  const roving = useRoving(ids)
  const list = useRef<HTMLUListElement>(null)
  const focusAfter = useRef<ObjectId | null>(null)
  const entryOf = useMemo(() => entriesById(stacks), [stacks])
  const width = box?.width
  const height = box?.height
  const style = useMemo<CSSProperties | undefined>(() => {
    if (width === undefined || height === undefined) return undefined
    const vars: Record<string, string> = { '--card-w': `${width}px`, '--card-h': `${height}px` }
    if (gap !== undefined) vars['--card-gap'] = `${gap}px`
    return vars as CSSProperties
  }, [width, height, gap])
  // Opening or closing swaps the stack's button for its members or back; the focus goes to the first card.
  useLayoutEffect(() => {
    const id = focusAfter.current
    if (id === null) return
    focusAfter.current = null
    list.current?.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()
  })
  const toggle = useCallback(
    (id: ObjectId) => {
      const first = entryOf.get(id)?.ids[0]
      if (first === undefined) return
      focusAfter.current = first
      onToggleStack?.(first)
    },
    [entryOf, onToggleStack],
  )
  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    roving.onKeyDown(event)
    if (event.key !== 'Escape' || !(event.target instanceof Element)) return
    const focused = objectIdOf(event.target)
    const entry = focused === null ? undefined : entryOf.get(focused)
    if (focused !== null && entry?.open === true && !entry.held) toggle(focused)
  }
  if (ids.length === 0 && ghosts.length === 0) return null
  const items = ghosts.length === 0 ? ids : withGhosts(ids, ghosts)
  const fit = box !== undefined
  const spaced = fit && gap !== undefined
  return (
    <ul
      ref={list}
      role="list"
      aria-label={label}
      // A row holding only ghosts has nothing to announce.
      aria-hidden={ids.length === 0 || undefined}
      onFocus={roving.onFocus}
      onKeyDown={onKeyDown}
      style={style}
      className={cn(
        // A fitted row keeps the gaps `fit.ts` counts (6 px across, 4 px between lines); the zone around it has the padding.
        spaced ? 'flex gap-0' : fit ? 'flex gap-x-1.5 gap-y-1' : 'flex gap-1.5 p-1',
        single ? 'overflow-x-auto' : 'flex-wrap',
        className,
      )}
    >
      {items.map((item) => {
        if (typeof item !== 'number') {
          return <GhostCard key={`ghost-${item.token}`} ghost={item} size={size} fit={fit} />
        }
        const entry = entryOf.get(item)
        const tabbable = item === roving.current
        const itemClass = cn(
          'flex',
          spaced && 'relative hover:z-10 focus-within:z-10 [&:not(:first-child)]:ml-(--card-gap)',
        )
        if (entry !== undefined && !entry.open) {
          return (
            <li key={item} className={itemClass}>
              <CardStack
                id={item}
                count={entry.ids.length}
                size={size}
                fit={fit}
                tabbable={tabbable}
                onOpen={toggle}
              />
            </li>
          )
        }
        // The first member of an open stack carries the chip that closes it, unless a prompt needs every member.
        const closes = entry !== undefined && !entry.held && entry.ids[0] === item
        return (
          <li
            key={item}
            data-stack-open={entry !== undefined && entry.ids[0] === item ? entry.ids.length : undefined}
            className={cn(itemClass, closes && 'relative')}
          >
            <Card id={item} size={size} fit={fit} tabbable={tabbable} />
            {closes ? (
              <button
                type="button"
                tabIndex={-1}
                aria-label={`Close the stack of ${entry.ids.length}`}
                aria-expanded={true}
                onClick={() => toggle(item)}
                className="absolute -bottom-1 -left-1 rounded-full bg-foreground px-1.5 text-[0.6875rem] leading-4 font-semibold text-background tabular-nums ring-2 ring-primary"
              >
                ×{entry.ids.length}
              </button>
            ) : null}
          </li>
        )
      })}
    </ul>
  )
}

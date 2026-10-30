import { useCallback, useLayoutEffect, useMemo, useRef } from 'react'
import type { CSSProperties, KeyboardEvent } from 'react'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { CardStack } from './CardStack'
import { CardWidthContext } from './ImageFace'
import { GhostCard } from './fx/GhostCard'
import { withGhosts } from './fx/ghosts'
import { NO_GHOSTS } from './fx/model'
import type { Ghost } from './fx/model'
import type { RowEntry } from './rowStacks'
import { TuckedCard } from './TuckedCard'
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
  /**
   * Fanned piles of `pile` cards each, filled pile by pile: every card lies over the one before it but its top `head`
   * px (the name line). Only with `box`.
   */
  fan?: { pile: number; head: number } | undefined
  /**
   * The tuck of a battlefield lane (`LanePlace.tuck` and `lift`): every box is `lift` px taller, the card standing at
   * its foot, and each attachment tucked behind a host (`RowEntry.tucked`) shows its top `each` px above the next.
   * Only with `box` and `stacks`.
   */
  tuck?: { each: number; lift: number } | undefined
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

/** Every host with attachments tucked behind it, by id, to those attachments. */
function tuckedById(stacks: readonly RowEntry[] | undefined): ReadonlyMap<ObjectId, readonly ObjectId[]> {
  const byId = new Map<ObjectId, readonly ObjectId[]>()
  for (const entry of stacks ?? []) {
    const host = entry.ids[0]
    if (entry.tucked !== undefined && entry.tucked.length > 0 && host !== undefined)
      byId.set(host, entry.tucked)
  }
  return byId
}

interface TuckedProps {
  ids: readonly ObjectId[]
  size: CardSize
  each: number
  lift: number
  /** The roving tab stop of the row, which may be one of them. */
  current: ObjectId | null
}

/**
 * The attachments behind a host, deepest first: the last lies `each` px above the host, every other `each` px above
 * the one after it, so each shows its name line. They keep their own cards, which a prompt marks and a click picks.
 */
function TuckedCards({ ids, size, each, lift, current }: TuckedProps) {
  return ids.map((id, at) => (
    <TuckedCard
      key={id}
      id={id}
      size={size}
      tabbable={id === current}
      style={{ top: lift - (ids.length - at) * each, zIndex: at }}
    />
  ))
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
  fan,
  tuck,
  single = false,
  className,
}: CardRowProps) {
  const roving = useRoving(ids)
  const list = useRef<HTMLUListElement>(null)
  const focusAfter = useRef<ObjectId | null>(null)
  const entryOf = useMemo(() => entriesById(stacks), [stacks])
  const tuckedOf = useMemo(() => tuckedById(stacks), [stacks])
  const behind = useMemo(() => new Set([...tuckedOf.values()].flat()), [tuckedOf])
  const width = box?.width
  const height = box?.height
  const style = useMemo<CSSProperties | undefined>(() => {
    if (width === undefined || height === undefined) return undefined
    const vars: Record<string, string> = { '--card-w': `${width}px`, '--card-h': `${height}px` }
    if (gap !== undefined) vars['--card-gap'] = `${gap}px`
    if (fan === undefined || fan.pile < 2) return vars as CSSProperties
    return {
      ...vars,
      gridTemplateRows: `repeat(${fan.pile - 1}, ${fan.head}px) ${height}px`,
    } as CSSProperties
  }, [width, height, gap, fan])
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
  const shown = behind.size === 0 ? ids : ids.filter((id) => !behind.has(id))
  const items = ghosts.length === 0 ? shown : withGhosts(shown, ghosts)
  const fit = box !== undefined
  const lifted = fit && tuck !== undefined && tuck.lift > 0 ? tuck : undefined
  const spaced = fit && gap !== undefined
  const fanned = fit && fan !== undefined && fan.pile > 1
  return (
    <CardWidthContext.Provider value={width ?? null}>
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
          fanned
            ? 'grid grid-flow-col items-start gap-x-1.5'
            : spaced
              ? 'flex gap-0'
              : fit
                ? 'flex gap-x-1.5 gap-y-1'
                : 'flex gap-1.5 p-1',
          !fanned && (single ? 'overflow-x-auto' : 'flex-wrap'),
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
            // No raise on hover: a raised card would cover the head of the next one under the pointer.
            fanned && 'relative has-focus-visible:z-10',
            // Every box of a lane with a tuck is as tall, so the cards of a line stand on one foot.
            lifted !== undefined && 'relative isolate',
          )
          const behindHost = tuckedOf.get(item)
          if (entry !== undefined && !entry.open) {
            return (
              <li
                key={item}
                className={itemClass}
                style={lifted === undefined ? undefined : { paddingTop: lifted.lift }}
              >
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
              style={lifted === undefined ? undefined : { paddingTop: lifted.lift }}
            >
              {lifted !== undefined && behindHost !== undefined ? (
                <TuckedCards
                  ids={behindHost}
                  size={size}
                  each={lifted.each}
                  lift={lifted.lift}
                  current={roving.current}
                />
              ) : null}
              {behindHost === undefined ? (
                <Card id={item} size={size} fit={fit} tabbable={tabbable} />
              ) : (
                <div
                  data-host={item}
                  className="relative flex rounded-md bg-lane"
                  style={{ zIndex: behindHost.length }}
                >
                  <Card id={item} size={size} fit={fit} tabbable={tabbable} />
                </div>
              )}
              {lifted === undefined && behindHost !== undefined
                ? // Before the zone is measured there is no tuck to draw: the attachments follow their host.
                  behindHost.map((id) => (
                    <Card key={id} id={id} size={size} fit={fit} tabbable={id === roving.current} />
                  ))
                : null}
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
    </CardWidthContext.Provider>
  )
}

import { useMemo } from 'react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { GhostCard } from './fx/GhostCard'
import { withGhosts } from './fx/ghosts'
import { NO_GHOSTS } from './fx/model'
import type { Ghost } from './fx/model'
import { useRoving } from './useRoving'

interface CardRowProps {
  ids: readonly ObjectId[]
  /** Permanents that just died, drawn in their old places while their effect plays; decoration, never in `ids`. */
  ghosts?: readonly Ghost[]
  /** Names the list for a screen reader, such as "Alice's creatures". */
  label: string
  /** Which parts of a face are drawn; the box too, unless `box` fits it. */
  size: CardSize
  /**
   * The box of every card in the row in px, from the fit of its zone (`useZoneFit`): written as `--card-w` and
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

/** A list of cards with one tab stop: Tab enters it, the arrow keys, Home and End move along it. */
export function CardRow({
  ids,
  ghosts = NO_GHOSTS,
  label,
  size,
  box,
  gap,
  single = false,
  className,
}: CardRowProps) {
  const roving = useRoving(ids)
  const width = box?.width
  const height = box?.height
  const style = useMemo<CSSProperties | undefined>(() => {
    if (width === undefined || height === undefined) return undefined
    const vars: Record<string, string> = { '--card-w': `${width}px`, '--card-h': `${height}px` }
    if (gap !== undefined) vars['--card-gap'] = `${gap}px`
    return vars as CSSProperties
  }, [width, height, gap])
  if (ids.length === 0 && ghosts.length === 0) return null
  const items = ghosts.length === 0 ? ids : withGhosts(ids, ghosts)
  const fit = box !== undefined
  const spaced = fit && gap !== undefined
  return (
    <ul
      role="list"
      aria-label={label}
      // A row holding only ghosts has nothing to announce.
      aria-hidden={ids.length === 0 || undefined}
      onFocus={roving.onFocus}
      onKeyDown={roving.onKeyDown}
      style={style}
      className={cn(
        // A fitted row keeps the gaps `fit.ts` counts (6 px across, 4 px between lines); the zone around it has the padding.
        spaced ? 'flex gap-0' : fit ? 'flex gap-x-1.5 gap-y-1' : 'flex gap-1.5 p-1',
        single ? 'overflow-x-auto' : 'flex-wrap',
        className,
      )}
    >
      {items.map((item) =>
        typeof item === 'number' ? (
          <li
            key={item}
            className={cn(
              'flex',
              spaced && 'relative hover:z-10 focus-within:z-10 [&:not(:first-child)]:ml-(--card-gap)',
            )}
          >
            <Card id={item} size={size} fit={fit} tabbable={item === roving.current} />
          </li>
        ) : (
          <GhostCard key={`ghost-${item.token}`} ghost={item} size={size} fit={fit} />
        ),
      )}
    </ul>
  )
}

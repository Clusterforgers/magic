import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import type { HandLayout } from './handFit'
import { useRoving } from './useRoving'

interface HandFanProps {
  /** The cards of the hand, in order. */
  ids: readonly ObjectId[]
  /** Names the list for a screen reader. */
  label: string
  /** Which parts of a face are drawn; the box too, unless `layout` fits it. */
  size: CardSize
  /** The box, gap and lift from the dock's room; without it the cards take their usual size, side by side. */
  layout?: HandLayout | undefined
}

/**
 * The cards of the hand as a fan: neighbours overlap, later ones over earlier, and a hovered or keyboard-focused
 * card comes to the front and rises by its lift (a fraction of its height, `HAND_LIFT`). The list has one tab stop and
 * the arrow keys, Home and End move along it. It is as tall as the cards plus the lift and stands on the bottom of the
 * dock: the part above the cards takes no pointer, so the table under it stays clickable, and a fan that scrolls
 * keeps the lifted card inside its own box.
 */
export function HandFan({ ids, label, size, layout }: HandFanProps) {
  const roving = useRoving(ids)
  const style =
    layout === undefined
      ? undefined
      : ({
          '--card-w': `${layout.box.width}px`,
          '--card-h': `${layout.box.height}px`,
          '--card-gap': `${layout.gap}px`,
          '--hand-lift': `${layout.lift}px`,
        } as CSSProperties)
  return (
    <ul
      role="list"
      aria-label={label}
      data-slot="hand-fan"
      data-scrolls={layout?.scrolls || undefined}
      onFocus={roving.onFocus}
      onKeyDown={roving.onKeyDown}
      style={style}
      className={cn(
        'pointer-events-none flex max-w-full items-end overflow-x-auto overflow-y-hidden px-1 pb-1 [scrollbar-width:thin]',
        layout === undefined ? 'gap-1.5 pt-1' : 'pt-(--hand-lift)',
      )}
    >
      {ids.map((id) => (
        <li
          key={id}
          data-slot="hand-slot"
          // The slot stays put under the pointer while its card rises, so the hover never lets go of it.
          className={cn(
            'group/slot pointer-events-auto relative flex hover:z-10 focus-within:z-10',
            layout !== undefined && '[&:not(:first-child)]:ml-(--card-gap)',
          )}
        >
          <div className="motion-safe:transition-transform motion-safe:duration-fast motion-safe:ease-out group-hover/slot:-translate-y-[40%] group-has-focus-visible/slot:-translate-y-[40%] pointer-coarse:group-focus-within/slot:-translate-y-[40%]">
            <Card id={id} size={size} fit={layout !== undefined} tabbable={id === roving.current} />
          </div>
        </li>
      ))}
    </ul>
  )
}

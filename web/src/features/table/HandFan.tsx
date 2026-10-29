import { Crown } from 'lucide-react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { HAND_HEAD_PX } from './handFit'
import type { HandLayout } from './handFit'
import { ManaPips } from './ManaPips'
import { CardWidthContext } from './ImageFace'
import { useRoving } from './useRoving'

interface HandFanProps {
  /** The cards of the hand, in order. */
  ids: readonly ObjectId[]
  /** Names the list for a screen reader. */
  label: string
  /** Which parts of a face are drawn; the box too, unless `layout` fits it. */
  size: CardSize
  /** The face draws the mana cost even when `size` is compact. */
  cost?: boolean
  /** The box, gap, rows and lift from the dock's room; without it the cards take their usual size, side by side. */
  layout?: HandLayout | undefined
  /** Cards that wait in the command zone, by id: the tax their cast costs more and the words that say so. */
  commanders?: ReadonlyMap<ObjectId, HandCommander>
}

/** A commander at the head of the hand: the tax on its cast and the words its spoken name gets. */
export interface HandCommander {
  tax: number
  words: string
}

/**
 * The cards of the hand: one row while they fit, else two or three, each row lying over the one before it so that its
 * top (name and cost) shows. A hovered or keyboard-focused card comes to the front and a card of a row another row
 * lies over rises by its lift (a fraction of its height, `HAND_LIFT`, at least what shows it whole). The list has one
 * tab stop and the arrow keys, Home and End move along it in reading order. It is as tall as the rows plus the lift and
 * stands on the bottom of the dock: the part above the cards takes no pointer, so the table under it stays clickable,
 * and a line that scrolls keeps the lifted card inside its own box.
 */
export function HandFan({ ids, label, size, cost = false, layout, commanders }: HandFanProps) {
  const roving = useRoving(ids)
  const rows = layout?.rows ?? 1
  const perRow = layout?.perRow ?? ids.length
  const style =
    layout === undefined
      ? undefined
      : ({
          '--card-w': `${layout.box.width}px`,
          '--card-h': `${layout.box.height}px`,
          '--card-gap': `${layout.gap}px`,
          '--hand-lift': `${rows > 1 ? Math.max(layout.lift, layout.overlap) : layout.lift}px`,
          ...(rows > 1 ? { width: `${perRow * layout.box.width + (perRow - 1) * layout.gap + 8}px` } : null),
        } as CSSProperties)
  return (
    <CardWidthContext.Provider value={layout?.box.width ?? null}>
      <ul
        role="list"
        aria-label={label}
        data-slot="hand-fan"
        data-scrolls={layout?.scrolls || undefined}
        data-rows={rows}
        onFocus={roving.onFocus}
        onKeyDown={roving.onKeyDown}
        style={style}
        className={cn(
          'pointer-events-none flex max-w-full items-end overflow-x-auto overflow-y-hidden px-1 pb-1 [scrollbar-width:thin]',
          layout === undefined ? 'gap-1.5 pt-1' : 'gap-x-(--card-gap) pt-(--hand-lift)',
          rows > 1 && 'flex-wrap content-start justify-center',
          layout?.scrolls === true && 'pb-3',
        )}
      >
        {ids.map((id, index) => {
          // A row another row lies over keeps only its top as its slot, so the slots of the row below start right under it.
          const back = rows > 1 && Math.floor(index / perRow) < rows - 1
          const commander = commanders?.get(id)
          return (
            <li
              key={id}
              data-slot="hand-slot"
              data-back={back || undefined}
              data-commander={commander === undefined ? undefined : true}
              style={back ? { height: HAND_HEAD_PX } : undefined}
              // The slot stays put under the pointer while its card rises, so the hover never lets go of it.
              className="group/slot pointer-events-auto relative flex items-start hover:z-10 focus-within:z-10"
            >
              {/* Faces are tinted and see-through; the backing keeps an overlapped neighbour from showing through. */}
              <div
                className={cn(
                  'relative rounded-md bg-table motion-safe:transition-transform motion-safe:duration-fast motion-safe:ease-out',
                  // The row nothing lies over is whole already; a lift there would only cover the row above.
                  (rows === 1 || back) &&
                    'group-hover/slot:-translate-y-(--hand-lift) group-has-focus-visible/slot:-translate-y-(--hand-lift) pointer-coarse:group-focus-within/slot:-translate-y-(--hand-lift)',
                )}
              >
                <Card
                  id={id}
                  size={size}
                  cost={cost}
                  fit={layout !== undefined}
                  tabbable={id === roving.current}
                  words={commander?.words}
                />
                {commander === undefined ? null : <TaxPill tax={commander.tax} />}
              </div>
            </li>
          )
        })}
      </ul>
    </CardWidthContext.Provider>
  )
}

/** The mark on a commander at the head of the hand: a crown, and the tax its cast costs more ("+{2}") while there is one. */
function TaxPill({ tax }: { tax: number }) {
  return (
    <span
      aria-hidden="true"
      data-slot="hand-commander-tax"
      className="pointer-events-none absolute bottom-1 left-1/2 flex -translate-x-1/2 items-center gap-0.5 rounded-full bg-background/90 px-1 py-px text-[0.6875rem] leading-4 font-semibold whitespace-nowrap text-foreground shadow-sm ring-1 ring-foreground/20"
    >
      <Crown className="size-3 shrink-0 text-primary" />
      {tax === 0 ? null : (
        <>
          +
          <ManaPips
            pips={[
              { key: 'tax', tone: 'generic', text: `${tax}`, symbol: `${tax}`, name: `${tax} generic mana` },
            ]}
            sizeClass="size-4 text-[0.6875rem]"
            label="Tax"
          />
        </>
      )}
    </span>
  )
}

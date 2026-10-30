import { memo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { cardLabel, costPips } from './cardModel'
import { SIZE } from './cardSize'
import { ManaPips } from './ManaPips'
import { useSeatNamer } from './names'
import { SeatDot } from './PodParts'
import { stackTopFirst } from './selectors'
import { hasNoLegalTarget } from './stackTargets'
import { targetsAria, targetsLine } from './stackText'
import { useRoving } from './useRoving'
import { usePreviewHandlers } from './usePreviewHandlers'
import { useStackEntries, useTargetNamer } from './useStackEntries'

interface ChipProps {
  id: ObjectId
  top: boolean
  tabbable: boolean
  /** The item's targets as a short line ("Hill Giant, Bo"), "no legal target", or nothing for an item that aims at nothing. */
  aims: string
  /** What a screen reader hears after the label: ", targeting Hill Giant and Bo". */
  aria: string
  /** Everything the item aims at is gone. */
  empty: boolean
}

const Chip = memo(function Chip({ id, top, tabbable, aims, aria, empty }: ChipProps) {
  const object = usePlay((state) => state.objectsById[id])
  const name = useSeatNamer()
  const handlers = usePreviewHandlers(id)
  if (object === undefined) return null
  return (
    <li className="shrink-0">
      <button
        type="button"
        data-object-id={id}
        data-stack-chip={id}
        data-no-target={empty || undefined}
        aria-label={`${top ? 'Top of the stack: ' : ''}${cardLabel(object, null)}, cast by ${name(object.controller)}${aria}`}
        tabIndex={tabbable ? 0 : -1}
        {...handlers}
        className={cn(
          'flex h-8 items-center gap-1.5 rounded-lg border bg-background px-2 text-sm font-medium outline-offset-2 outline-primary focus-visible:outline-2',
          top ? 'max-w-72 border-primary' : 'max-w-48',
          empty && 'border-dashed',
        )}
      >
        {top ? <SeatDot seat={object.controller} /> : null}
        <span className="truncate">{object.name}</span>
        {object.mana_cost === null ? null : (
          <ManaPips pips={costPips(object.mana_cost)} sizeClass={SIZE.compact.pip} label="Mana cost" />
        )}
        {top && aims !== '' ? (
          <span
            data-slot="chip-targets"
            aria-hidden="true"
            className={cn(
              'min-w-0 truncate text-xs font-normal',
              empty ? 'text-destructive' : 'text-muted-foreground',
            )}
          >
            → {aims}
          </span>
        ) : null}
      </button>
    </li>
  )
})

/**
 * The spells on the stack as chips in the shared strip, the one that resolves first leading and ringed. One tab stop,
 * moved with the arrow keys; hover or focus previews the card. Nothing is drawn while the stack is empty. The Stack tab
 * of the side area has the same spells with their controllers and types. The top chip names what it aims at ("→ Hill
 * Giant") beside a dot in its controller's colour and is where the arrows to the targets start; a chip that is
 * hovered or focused draws its own arrows (`combat/stackArrows.ts`).
 */
export function StackChips() {
  const ids = usePlay(useShallow(stackTopFirst))
  const roving = useRoving(ids)
  const entries = useStackEntries()
  const names = useTargetNamer()
  if (ids.length === 0) return null
  return (
    <div data-slot="stack-chips" className="flex min-w-0 shrink items-center gap-2">
      <span className="text-xs font-medium text-muted-foreground">Stack</span>
      <ol
        role="list"
        aria-label="Spells on the stack, top first"
        onFocus={roving.onFocus}
        onKeyDown={roving.onKeyDown}
        className="flex min-w-0 items-center gap-1.5 overflow-x-auto p-1 [scrollbar-width:none]"
      >
        {ids.map((id, index) => {
          const entry = entries.find((candidate) => candidate.id === id)
          return (
            <Chip
              key={id}
              id={id}
              top={index === 0}
              tabbable={id === roving.current}
              aims={entry === undefined ? '' : targetsLine(entry, names)}
              aria={entry === undefined ? '' : targetsAria(entry, names)}
              empty={entry !== undefined && hasNoLegalTarget(entry)}
            />
          )
        })}
      </ol>
    </div>
  )
}

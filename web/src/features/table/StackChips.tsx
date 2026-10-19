import { memo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { cardLabel, costPips } from './cardModel'
import { SIZE } from './cardSize'
import { ManaPips } from './ManaPips'
import { useSeatNamer } from './names'
import { stackTopFirst } from './selectors'
import { useRoving } from './useRoving'
import { usePreviewHandlers } from './usePreviewHandlers'

const Chip = memo(function Chip({ id, top, tabbable }: { id: ObjectId; top: boolean; tabbable: boolean }) {
  const object = usePlay((state) => state.objectsById[id])
  const name = useSeatNamer()
  const handlers = usePreviewHandlers(id)
  if (object === undefined) return null
  return (
    <li className="shrink-0">
      <button
        type="button"
        data-object-id={id}
        aria-label={`${top ? 'Top of the stack: ' : ''}${cardLabel(object, null)}, cast by ${name(object.controller)}`}
        tabIndex={tabbable ? 0 : -1}
        {...handlers}
        className={cn(
          'flex h-8 max-w-48 items-center gap-1.5 rounded-lg border bg-background px-2 text-sm font-medium',
          top && 'border-primary',
        )}
      >
        <span className="truncate">{object.name}</span>
        {object.mana_cost === null ? null : (
          <ManaPips pips={costPips(object.mana_cost)} sizeClass={SIZE.compact.pip} label="Mana cost" />
        )}
      </button>
    </li>
  )
})

/**
 * The spells on the stack as chips in the shared strip, the one that resolves first leading and ringed. One tab stop,
 * moved with the arrow keys; hover or focus previews the card. Nothing is drawn while the stack is empty. The Stack tab
 * of the side area has the same spells with their controllers and types.
 */
export function StackChips() {
  const ids = usePlay(useShallow(stackTopFirst))
  const roving = useRoving(ids)
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
        {ids.map((id, index) => (
          <Chip key={id} id={id} top={index === 0} tabbable={id === roving.current} />
        ))}
      </ol>
    </div>
  )
}

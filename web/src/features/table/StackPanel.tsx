import { memo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { cardLabel, costPips, typeLine } from './cardModel'
import { SIZE } from './cardSize'
import { ManaPips } from './ManaPips'
import { useSeatNamer } from './names'
import { stackTopFirst } from './selectors'
import { useRoving } from './useRoving'
import { usePreviewHandlers } from './usePreviewHandlers'

const StackItem = memo(function StackItem({
  id,
  top,
  tabbable,
}: {
  id: ObjectId
  top: boolean
  tabbable: boolean
}) {
  const object = usePlay((state) => state.objectsById[id])
  const name = useSeatNamer()
  const handlers = usePreviewHandlers(id)
  if (object === undefined) return null
  const controller = name(object.controller)
  return (
    <li>
      <button
        type="button"
        data-object-id={id}
        aria-label={`${top ? 'Top of the stack: ' : ''}${cardLabel(object, null)}, cast by ${controller}`}
        tabIndex={tabbable ? 0 : -1}
        {...handlers}
        className={cn(
          'grid w-full gap-0.5 rounded-lg border bg-background px-2.5 py-1.5 text-left text-sm outline-offset-2 outline-primary focus-visible:outline-2',
          top && 'border-primary',
        )}
      >
        <span className="flex flex-wrap items-center justify-between gap-x-2">
          <span className="font-semibold">{object.name}</span>
          {object.mana_cost === null ? null : (
            <ManaPips pips={costPips(object.mana_cost)} sizeClass={SIZE.normal.pip} label="Mana cost" />
          )}
        </span>
        <span className="text-xs text-muted-foreground">
          {typeLine(object)} · cast by {controller}
          {top ? ' · resolves first' : ''}
        </span>
      </button>
    </li>
  )
})

/** The stack, the next spell to resolve first, or a plain note when it is empty. */
export function StackPanel() {
  const ids = usePlay(useShallow(stackTopFirst))
  const roving = useRoving(ids)
  return (
    <section
      aria-label="Stack"
      className="grid content-start gap-2 rounded-xl bg-card p-3 ring-1 ring-foreground/10"
    >
      <h2 className="text-sm font-semibold">
        Stack <span className="font-normal text-muted-foreground">({ids.length})</span>
      </h2>
      {ids.length === 0 ? (
        <p className="text-sm text-muted-foreground">The stack is empty.</p>
      ) : (
        <ol
          role="list"
          aria-label="Stack, top first"
          onFocus={roving.onFocus}
          onKeyDown={roving.onKeyDown}
          className="grid gap-1.5"
        >
          {ids.map((id, index) => (
            <StackItem key={id} id={id} top={index === 0} tabbable={id === roving.current} />
          ))}
        </ol>
      )}
    </section>
  )
}

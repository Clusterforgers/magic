import { Zap } from 'lucide-react'
import { memo, useContext, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { cardLabel, costPips } from './cardModel'
import { SIZE } from './cardSize'
import { FxMark } from './fx/FxMark'
import { useCardFx, useStackGhosts } from './fx/hooks'
import { withGhosts } from './fx/ghosts'
import { StackGhostChip } from './StackGhostChip'
import { isMarkFx } from './fx/model'
import { ByHandMark } from './manual/ByHand'
import { ManaPips } from './ManaPips'
import { useSeatNamer } from './names'
import { SeatDot } from './PodParts'
import { stackTopFirst } from './selectors'
import { SideContext } from './sideContext'
import { chipsThatFit, groupStack } from './stackGroups'
import type { StackGroup } from './stackGroups'
import { hasNoLegalTarget } from './stackTargets'
import { targetsAria, targetsLine } from './stackText'
import { useRoving } from './useRoving'
import { usePreviewHandlers } from './usePreviewHandlers'
import { useStackEntries, useTargetNamer } from './useStackEntries'

interface ChipProps {
  group: StackGroup
  top: boolean
  tabbable: boolean
  /** Past the room of the strip: kept in place to be measured, but neither seen, heard nor reached. */
  hidden: boolean
  /** The item's targets as a short line ("Hill Giant, Bo"), "no legal target", or nothing for an item that aims at nothing. */
  aims: string
  /** What a screen reader hears after the label: ", targeting Hill Giant and Bo". */
  aria: string
  /** Everything the item aims at is gone. */
  empty: boolean
}

const Chip = memo(function Chip({ group, top, tabbable, hidden, aims, aria, empty }: ChipProps) {
  const id = group.ids[0] ?? 0
  const count = group.ids.length
  const object = usePlay((state) => state.objectsById[id])
  const name = useSeatNamer()
  const fx = useCardFx(id)
  const handlers = usePreviewHandlers(id)
  if (object === undefined) return null
  const ability = object.kind === 'Ability'
  // An ability has no cost or types to read out; its text says what it is.
  const what = ability
    ? `Ability of ${object.name}${object.rules_text === undefined ? '' : ` (${object.rules_text})`}, controlled`
    : `${cardLabel(object, null)}, cast`
  return (
    <li className={cn('shrink-0', hidden && 'invisible')} aria-hidden={hidden || undefined}>
      <button
        type="button"
        data-object-id={hidden ? undefined : id}
        data-stack-chip={hidden ? undefined : id}
        data-stack-members={hidden || count === 1 ? undefined : group.ids.join(' ')}
        data-count={count > 1 ? count : undefined}
        data-no-target={empty || undefined}
        aria-label={`${top ? 'Top of the stack: ' : ''}${what} by ${name(object.controller)}${count > 1 ? `, ${count} times` : ''}${aria}`}
        tabIndex={tabbable && !hidden ? 0 : -1}
        {...(hidden ? {} : handlers)}
        className={cn(
          'relative flex h-8 items-center gap-1.5 rounded-lg border bg-background px-2 text-sm font-medium outline-offset-2 outline-primary focus-visible:outline-2',
          top ? 'max-w-72 border-primary' : 'max-w-48',
          empty && 'border-dashed',
        )}
      >
        {top ? <SeatDot seat={object.controller} /> : null}
        {ability ? <Zap aria-hidden="true" className="size-3.5 shrink-0 text-muted-foreground" /> : null}
        <span className="truncate">{object.name}</span>
        {count > 1 ? (
          <span
            data-slot="chip-count"
            aria-hidden="true"
            className="shrink-0 rounded-sm bg-muted px-1 text-xs font-semibold tabular-nums"
          >
            ×{count}
          </span>
        ) : null}
        <ByHandMark object={object} spot="chip" compact text="text-xs" />
        {object.mana_cost === null ? null : (
          <ManaPips pips={costPips(object.mana_cost)} sizeClass={SIZE.compact.pip} label="Mana cost" />
        )}
        {fx.map((entry) =>
          isMarkFx(entry.kind) ? <FxMark key={entry.token} kind={entry.kind} inline /> : null,
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

/** How many children of `element` fit its width, from their boxes on screen. */
function fitOf(element: HTMLElement): number {
  const start = element.getBoundingClientRect().left
  const rights = [...element.children].map((child) => child.getBoundingClientRect().right - start)
  return chipsThatFit(rights, element.clientWidth)
}

/**
 * How many children of `list` fit its width, measured after every layout and resize; all of them where there is no
 * layout (a test).
 */
function useFitCount(list: RefObject<HTMLOListElement | null>, items: number): number {
  const [fit, setFit] = useState(items)
  // A chip changes width with its count and marks, so every render measures again; the same answer redraws nothing.
  useLayoutEffect(() => {
    const element = list.current
    if (element !== null) setFit(fitOf(element))
  })
  const shown = items > 0
  useLayoutEffect(() => {
    const element = list.current
    if (!shown || element === null || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(() => setFit(fitOf(element)))
    observer.observe(element)
    return () => observer.disconnect()
  }, [list, shown])
  return Math.min(fit, items)
}

/**
 * The spells and abilities on the stack as chips in the shared strip, the one that resolves first leading and
 * ringed. Items next to each other that are the same ability of one source (a board of triggers) fold into one chip
 * with a count ("×6", `stackGroups.ts`). The chips that do not fit the strip give way to one "+N more" button, which
 * opens the Stack tab, so the row never scrolls or runs out of the strip. One tab stop, moved with the arrow keys;
 * hover or focus previews the card. Nothing is drawn while the stack is empty. The top chip names what it aims at
 * ("→ Hill Giant") beside a dot in its controller's colour and is where the arrows to the targets start; a chip that
 * is hovered or focused draws its own arrows (`combat/stackArrows.ts`).
 */
export function StackChips() {
  const ids = usePlay(useShallow(stackTopFirst))
  const objectsById = usePlay((state) => state.objectsById)
  const groups = useMemo(() => groupStack(ids, objectsById), [ids, objectsById])
  const leaders = useMemo(() => groups.map((group) => group.ids[0] ?? 0), [groups])
  const entries = useStackEntries()
  const names = useTargetNamer()
  const items = withGhosts(leaders, useStackGhosts())
  const list = useRef<HTMLOListElement>(null)
  const fit = useFitCount(list, items.length)
  const shown = new Set(items.slice(0, fit).filter((item) => typeof item === 'number'))
  const roving = useRoving(leaders.filter((id) => shown.has(id)))
  const side = useContext(SideContext)
  if (items.length === 0) return null
  const folded = groups.filter((group) => !shown.has(group.ids[0] ?? 0)).flatMap((group) => group.ids)
  return (
    <div data-slot="stack-chips" className="flex min-w-0 shrink items-center gap-2">
      <span className="text-xs font-medium text-muted-foreground">Stack</span>
      <ol
        ref={list}
        role="list"
        aria-label="Spells on the stack, top first"
        onFocus={roving.onFocus}
        onKeyDown={roving.onKeyDown}
        className="flex min-w-0 items-center gap-1.5 overflow-hidden p-1"
      >
        {items.map((item, index) => {
          if (typeof item !== 'number') return <StackGhostChip key={`ghost:${item.token}`} ghost={item} />
          const group = groups.find((candidate) => candidate.ids[0] === item)
          if (group === undefined) return null
          const entry = entries.find((candidate) => candidate.id === item)
          return (
            <Chip
              key={group.key}
              group={group}
              top={item === leaders[0]}
              tabbable={item === roving.current}
              hidden={index >= fit}
              aims={entry === undefined ? '' : targetsLine(entry, names)}
              aria={entry === undefined ? '' : targetsAria(entry, names)}
              empty={entry !== undefined && hasNoLegalTarget(entry)}
            />
          )
        })}
      </ol>
      {folded.length === 0 ? null : (
        <button
          type="button"
          data-slot="stack-more"
          // An arrow to an item that is folded away ends here.
          data-stack-members={folded.join(' ')}
          aria-label={`${folded.length} more on the stack: open the Stack tab`}
          onClick={() => {
            if (side !== null && !(side.open && side.tab === 'stack')) side.activate('stack')
          }}
          className="h-8 shrink-0 rounded-lg border border-dashed px-2 text-sm font-medium text-muted-foreground outline-offset-2 outline-primary hover:text-foreground focus-visible:outline-2"
        >
          +{folded.length} more
        </button>
      )}
    </div>
  )
}

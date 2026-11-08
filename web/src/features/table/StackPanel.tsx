import { ChevronDown } from 'lucide-react'
import { memo, useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { CardFace } from './CardFace'
import { targetName } from './eventText'
import { cardLabel, costPips, typeLine } from './cardModel'
import { SIZE } from './cardSize'
import { ByHandMark } from './manual/ByHand'
import { ManaPips } from './ManaPips'
import { useSeatNamer } from './names'
import { SeatDot } from './PodParts'
import { objectWords } from './prompt/targetText'
import { stackTopFirst } from './selectors'
import { shownObject } from './stackPreview'
import { groupStack } from './stackGroups'
import type { StackGroup } from './stackGroups'
import { hasNoLegalTarget } from './stackTargets'
import type { StackEntry } from './stackTargets'
import { NO_LEGAL_TARGET, targetsAria, targetsLine } from './stackText'
import { useRoving } from './useRoving'
import { usePreviewHandlers } from './usePreviewHandlers'
import { useStackEntries, useTargetNamer } from './useStackEntries'

const ROW =
  'grid w-full gap-0.5 rounded-lg border bg-background px-2.5 py-1.5 text-left text-sm outline-offset-2 outline-primary focus-visible:outline-2'

/** Every target of the top item, one row each, with whose it is or where it is. */
function TargetList({ entry }: { entry: StackEntry }) {
  const objects = usePlay((state) => state.objectsById)
  const names = useTargetNamer()
  const seatName = useSeatNamer()
  if (entry.targets.length === 0) return null
  return (
    <ul aria-label="Targets" data-slot="stack-targets" className="grid gap-0.5 text-sm">
      {entry.targets.map(({ key, target, gone }) => {
        const aimed = 'Object' in target ? objects[target.Object.id] : undefined
        const text =
          aimed === undefined ? targetName(target, names) : objectWords(aimed, aimed.name, seatName)
        return (
          <li
            key={key}
            data-gone={gone || undefined}
            className={cn('flex gap-1.5', gone && 'text-muted-foreground line-through')}
          >
            <span aria-hidden="true">→</span>
            <span>
              {text}
              {gone ? <span className="sr-only"> (no longer a legal target)</span> : null}
            </span>
          </li>
        )
      })}
    </ul>
  )
}

const StackItem = memo(function StackItem({
  id,
  top,
  tabbable,
  entry,
  count = 1,
}: {
  id: ObjectId
  top: boolean
  tabbable: boolean
  entry: StackEntry | undefined
  /** How many items of the same make the group this one leads; 1 for an item alone. */
  count?: number
}) {
  const object = usePlay((state) => state.objectsById[id])
  const source = usePlay((state) =>
    object?.source === undefined ? undefined : state.objectsById[object.source],
  )
  const name = useSeatNamer()
  const targetNames = useTargetNamer()
  const handlers = usePreviewHandlers(id)
  if (object === undefined) return null
  const controller = name(object.controller)
  const aims = entry === undefined ? '' : targetsLine(entry, targetNames)
  const empty = entry !== undefined && hasNoLegalTarget(entry)
  const button = (
    <button
      type="button"
      data-object-id={id}
      data-no-target={empty || undefined}
      aria-label={`${top ? 'Top of the stack: ' : ''}${cardLabel(object, null)}, cast by ${controller}${entry === undefined ? '' : targetsAria(entry, targetNames)}`}
      tabIndex={tabbable ? 0 : -1}
      {...handlers}
      className={cn(ROW, top && 'border-primary', empty && 'border-dashed', top && 'bg-card')}
    >
      <span className="flex flex-wrap items-center justify-between gap-x-2">
        <span className={cn('flex items-center gap-1.5 font-semibold', top && 'text-base')}>
          {top ? <SeatDot seat={object.controller} /> : null}
          {object.name}
          {count > 1 ? (
            <span
              data-slot="row-count"
              className="rounded-sm bg-muted px-1 text-xs font-semibold tabular-nums"
            >
              ×{count}
            </span>
          ) : null}
          <ByHandMark object={object} spot="chip" compact={false} text="text-xs" />
        </span>
        {object.mana_cost === null ? null : (
          <ManaPips pips={costPips(object.mana_cost)} sizeClass={SIZE.normal.pip} label="Mana cost" />
        )}
      </span>
      <span className="text-xs text-muted-foreground">
        {/* An ability has no type line; its text says what it does. */}
        {object.kind === 'Ability'
          ? `${object.rules_text ?? 'Ability'} · controlled by ${controller}`
          : `${typeLine(object)} · cast by ${controller}`}
        {top ? ' · resolves first' : ''}
      </span>
      {top || aims === '' ? null : (
        <span
          data-slot="row-targets"
          className={cn('text-xs', empty ? 'text-destructive' : 'text-muted-foreground')}
        >
          → {aims}
        </span>
      )}
    </button>
  )
  if (!top) return <li>{button}</li>
  return (
    <li
      data-slot="stack-top"
      className="grid gap-2 @sm:grid-cols-[auto_1fr] @sm:items-start @md:col-span-full"
    >
      <span
        aria-hidden="true"
        className="relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center justify-self-center rounded-lg ring-1 ring-border-strong [--card-h:9.8rem] [--card-w:7rem]"
      >
        <CardFace object={shownObject(object, source)} role={null} size="large" fit />
      </span>
      <div className="grid min-w-0 content-start gap-2">
        {button}
        {entry === undefined ? null : <TargetList entry={entry} />}
        {empty ? (
          <p data-slot="no-target" className="text-xs text-destructive">
            {NO_LEGAL_TARGET} left: it does nothing when it resolves.
          </p>
        ) : null}
      </div>
    </li>
  )
})

/**
 * A group of items of the same make (`stackGroups.ts`): its first item with the count, and a button that lists the
 * others under it.
 */
function GroupItems({
  group,
  top,
  current,
  entries,
  open,
  onToggle,
}: {
  group: StackGroup
  top: boolean
  current: ObjectId | null
  entries: readonly StackEntry[]
  open: boolean
  onToggle: () => void
}) {
  const [first, ...rest] = group.ids
  if (first === undefined) return null
  const item = (id: ObjectId, lead: boolean) => (
    <StackItem
      key={id}
      id={id}
      top={lead && top}
      tabbable={id === current}
      entry={entries.find((entry) => entry.id === id)}
      count={lead ? group.ids.length : 1}
    />
  )
  return (
    <>
      {item(first, true)}
      {rest.length === 0 ? null : (
        <li data-slot="stack-group" className="grid gap-1.5 @md:col-span-full">
          <button
            type="button"
            aria-expanded={open}
            onClick={onToggle}
            className="flex w-fit items-center gap-1 rounded-md px-1.5 py-0.5 text-xs font-medium text-muted-foreground outline-offset-2 outline-primary hover:text-foreground focus-visible:outline-2"
          >
            <ChevronDown aria-hidden="true" className={cn('size-3.5', !open && '-rotate-90')} />
            {open ? 'Hide' : 'Show'} {rest.length} more the same
          </button>
          {open ? (
            <ol role="list" aria-label="The same, in order" className="grid gap-1.5 @md:grid-cols-2">
              {rest.map((id) => item(id, false))}
            </ol>
          ) : null}
        </li>
      )}
    </>
  )
}

/**
 * The stack, the next spell to resolve first and large with its targets, or a plain note when it is empty. Items of the
 * same make next to each other fold into their first with a count and a button that lists the rest.
 */
export function StackPanel() {
  const ids = usePlay(useShallow(stackTopFirst))
  const objectsById = usePlay((state) => state.objectsById)
  const groups = useMemo(() => groupStack(ids, objectsById), [ids, objectsById])
  const [opened, setOpened] = useState<ReadonlySet<string>>(() => new Set())
  const shown = useMemo(
    () => groups.flatMap((group) => (opened.has(group.key) ? group.ids : group.ids.slice(0, 1))),
    [groups, opened],
  )
  const roving = useRoving(shown)
  const entries = useStackEntries()
  const toggle = (key: string): void =>
    setOpened((was) => {
      const next = new Set(was)
      if (!next.delete(key)) next.add(key)
      return next
    })
  return (
    <section
      aria-label="Stack"
      className="@container grid content-start gap-2 rounded-xl bg-card p-3 ring-1 ring-foreground/10"
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
          className="grid gap-1.5 @md:grid-cols-2 @3xl:grid-cols-3"
        >
          {groups.map((group, index) => (
            <GroupItems
              key={group.key}
              group={group}
              top={index === 0}
              current={roving.current}
              entries={entries}
              open={opened.has(group.key)}
              onToggle={() => toggle(group.key)}
            />
          ))}
        </ol>
      )}
    </section>
  )
}

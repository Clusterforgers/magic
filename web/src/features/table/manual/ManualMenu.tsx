import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { CSSProperties, KeyboardEvent, ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { useShallow } from 'zustand/react/shallow'
import { useGame, usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { CounterKind, ManualAction } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import { playerOf } from '../selectors'
import { ManualForm } from './ManualForm'
import { ByHandList } from './ByHand'
import { ByHandPanel } from './ByHandPanel'
import { createMenuStore, ManualMenuContext, useMenuStore, useMenuTarget } from './menuStore'
import type { MenuTarget } from './menuStore'
import { GAME_OVER, manualMenu, OUT_OF_GAME } from './offers'
import type { Ask, ManualGroup, ManualItem } from './offers'
import { containerOf, useCloseOutside, usePlaced, useReturnFocus } from './popup'

const NO_COUNTERS: readonly (readonly [CounterKind, number])[] = []
function focusables(panel: HTMLElement): HTMLElement[] {
  return [...panel.querySelectorAll<HTMLElement>('[role="menuitem"]')]
}

interface ItemProps {
  item: ManualItem
  notice: string | undefined
  onUse: (item: ManualItem) => void
}

function Item({ item, notice, onUse }: ItemProps) {
  const reason = item.unavailable === notice ? undefined : item.unavailable
  const disabled = item.unavailable !== undefined
  return (
    <button
      type="button"
      role="menuitem"
      tabIndex={-1}
      aria-disabled={disabled || undefined}
      data-item={item.key}
      onClick={disabled ? undefined : () => onUse(item)}
      className={cn(
        'flex w-full min-w-0 flex-col rounded-md px-3 py-1.5 text-left text-sm outline-none select-none focus-visible:bg-accent focus-visible:text-accent-foreground pointer-coarse:min-h-11 pointer-coarse:justify-center',
        disabled ? 'cursor-not-allowed opacity-60' : 'hover:bg-accent hover:text-accent-foreground',
      )}
    >
      <span className="flex min-w-0 items-baseline justify-between gap-2">
        <span className="truncate">{item.label}</span>
        {item.hint === undefined ? null : (
          <span className="shrink-0 text-xs text-muted-foreground">{item.hint}</span>
        )}
      </span>
      {reason === undefined ? null : <span className="text-xs text-muted-foreground">{reason}</span>}
    </button>
  )
}

function Group({
  group,
  notice,
  onUse,
}: {
  group: ManualGroup
  notice: string | undefined
  onUse: ItemProps['onUse']
}) {
  const heading = `manual-group-${group.id}`
  const empty = group.items.length === 0 && group.note !== undefined
  return (
    <div role="group" aria-labelledby={heading} data-group={group.id} className="grid gap-0.5 py-1">
      <p id={heading} role="presentation" className="px-3 pt-1 text-xs font-medium text-muted-foreground">
        {group.label}
      </p>
      {empty ? (
        <Item
          item={{ key: `${group.id}-note`, label: group.note ?? '', unavailable: group.note }}
          notice={group.note}
          onUse={onUse}
        />
      ) : (
        group.items.map((item) => <Item key={item.key} item={item} notice={notice} onUse={onUse} />)
      )}
    </div>
  )
}

/** The menu of one card, open at `target`. */
function Panel({ target }: { target: MenuTarget }) {
  const store = useMenuStore()
  const { session } = useGame()
  const nameOf = useSeatNamer()
  const panel = useRef<HTMLDivElement>(null)
  const [ask, setAsk] = useState<Ask | null>(null)
  const [error, setError] = useState<string | undefined>(undefined)
  const seat = usePlay((state) => state.seat)
  const object = usePlay((state) => state.objectsById[target.id])
  const objects = usePlay((state) => state.objectsById)
  const hasCommandZone = usePlay((state) => state.view?.format.toLowerCase() === 'commander')
  const mine = usePlay(
    useShallow((state) => {
      const player = seat === null ? undefined : playerOf(state, seat)
      return {
        library: player?.library.count ?? 0,
        counters: player?.counters ?? NO_COUNTERS,
        off: state.result !== null ? GAME_OVER : (player?.lost ?? null) !== null ? OUT_OF_GAME : undefined,
      }
    }),
  )
  const players = usePlay(
    useShallow((state) => (state.view?.players ?? []).filter((p) => p.lost === null).map((p) => p.seat)),
  )
  const close = useCallback((): void => store?.close(), [store])

  const input = useMemo(
    () =>
      object === undefined || seat === null
        ? null
        : {
            seat,
            object,
            off: mine.off,
            players,
            hasCommandZone,
            library: mine.library,
            counters: mine.counters,
            ownerName: nameOf(object.owner),
          },
    [seat, object, players, hasCommandZone, mine, nameOf],
  )
  const menu = useMemo(() => (input === null ? null : manualMenu(input)), [input])

  // The card left the table (or the seat is gone) while the menu was open.
  const gone = input === null
  useEffect(() => {
    if (gone) store?.close()
  }, [gone, store])

  // The menu takes the focus and gives it back to the card that asked.
  useReturnFocus(target)
  useEffect(() => {
    const element = panel.current
    if (element === null) return
    const first =
      ask === null
        ? (element.querySelector<HTMLElement>('[role="menuitem"]:not([aria-disabled])') ??
          element.querySelector<HTMLElement>('[role="menuitem"]'))
        : element.querySelector<HTMLElement>('[data-autofocus]')
    first?.focus({ preventScroll: true })
  }, [ask, gone])
  useCloseOutside(panel, store)
  usePlaced(panel, target, ask)

  if (menu === null || input === null) return null

  // Sent at any time, apart from any decision; a refusal comes back as the session's notice with the engine's reason.
  const send = (action: ManualAction): void => {
    if (menu.notice !== undefined) setError(menu.notice)
    else if (session.manual(action)) close()
    else setError('The action could not be sent; the connection is not open.')
  }
  const use = (item: ManualItem): void => {
    setError(undefined)
    if (item.action !== undefined) send(item.action)
    else if (item.ask !== undefined) setAsk(item.ask)
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    event.stopPropagation()
    if (event.key === 'Escape') {
      event.preventDefault()
      if (ask === null) close()
      else setAsk(null)
      return
    }
    if (ask !== null) return
    if (event.key === 'Tab') {
      close()
      return
    }
    const items = focusables(event.currentTarget)
    const at = items.indexOf(document.activeElement as HTMLElement)
    const next =
      event.key === 'ArrowDown'
        ? (at + 1) % items.length
        : event.key === 'ArrowUp'
          ? (at - 1 + items.length) % items.length
          : event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? items.length - 1
              : -1
    if (next < 0) return
    event.preventDefault()
    items[next]?.focus({ preventScroll: true })
  }

  const body: ReactNode =
    ask === null ? (
      <div
        role="menu"
        aria-label={`Manual actions for ${menu.title}`}
        aria-describedby={menu.notice === undefined ? undefined : 'manual-notice'}
        className="min-h-0 overflow-y-auto overscroll-contain px-1 pb-1"
      >
        {menu.groups.map((group) => (
          <Group key={group.id} group={group} notice={menu.notice} onUse={use} />
        ))}
      </div>
    ) : (
      <div className="min-h-0 overflow-y-auto overscroll-contain">
        <ManualForm
          ask={ask}
          input={input}
          objects={objects}
          nameOf={nameOf}
          onBack={() => setAsk(null)}
          onSend={send}
        />
      </div>
    )
  return (
    <div
      ref={panel}
      data-slot="manual-menu"
      data-card={target.id}
      onKeyDown={onKeyDown}
      style={{ '--mx': `${target.x}px`, '--my': `${target.y}px` } as CSSProperties}
      className="fixed z-[70] flex max-h-[min(36rem,calc(100dvh-1rem))] w-72 max-w-[calc(100vw-1rem)] flex-col overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-e2 max-sm:inset-x-2 max-sm:bottom-2 max-sm:w-auto max-sm:max-h-[72dvh] sm:top-(--my) sm:left-(--mx)"
    >
      <div className="grid gap-0.5 border-b px-3 py-2">
        <p className="truncate text-sm font-medium">{menu.title}</p>
        {menu.notice === undefined ? null : (
          <p id="manual-notice" className="text-xs text-muted-foreground">
            {menu.notice}
          </p>
        )}
        {menu.clauses.length === 0 ? null : (
          <section
            aria-label="By hand"
            data-slot="by-hand"
            className="mt-1 grid max-h-28 gap-1 overflow-y-auto rounded-md bg-muted px-2 py-1.5"
          >
            <p className="text-xs font-medium">By hand</p>
            <ByHandList clauses={menu.clauses} className="text-xs" />
          </section>
        )}
        {error === undefined ? null : (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        )}
      </div>
      {body}
    </div>
  )
}

/** Draws the open menu, if any, on the page or inside the dialog its card sits in. */
export function ManualMenuHost() {
  const target = useMenuTarget()
  if (target === null) return null
  const key = `${target.kind ?? 'menu'}:${target.id}:${target.x}:${target.y}`
  return createPortal(
    target.kind === 'clauses' ? <ByHandPanel key={key} target={target} /> : <Panel key={key} target={target} />,
    containerOf(target),
  )
}

/** The menu store of a table and the host that draws its menu, around the cards. */
export function ManualMenuProvider({ children }: { children: ReactNode }) {
  const [store] = useState(createMenuStore)
  return (
    <ManualMenuContext value={store}>
      {children}
      <ManualMenuHost />
    </ManualMenuContext>
  )
}

import { HandIcon, LayersIcon, MessageSquareIcon, PanelRightIcon, ScrollTextIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { CSSProperties, KeyboardEvent } from 'react'
import { createPortal } from 'react-dom'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { Sheet, SheetContent, SheetTitle } from '@/components/ui/sheet'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { ChatPanel } from './ChatPanel'
import { LogPanel } from './LogPanel'
import { CommandsPanel } from './manual/panel/CommandsPanel'
import { useSide } from './sideContext'
import { countLabel } from './sideCounts'
import type { SideTab } from './sideCounts'
import { StackPanel } from './StackPanel'

const TABS: ReadonlyArray<{ tab: SideTab; label: string; Icon: LucideIcon }> = [
  { tab: 'commands', label: 'Commands', Icon: HandIcon },
  { tab: 'log', label: 'Log', Icon: ScrollTextIcon },
  { tab: 'chat', label: 'Chat', Icon: MessageSquareIcon },
  { tab: 'stack', label: 'Stack', Icon: LayersIcon },
]

/** The side area's name for a seat, which has Commands, and for a viewer that is no seat. */
export const SEATED_NAME = 'Commands, log, chat and stack'
const SPECTATOR_NAME = 'Log, chat and stack'

/** What a count means to a screen reader. */
const COUNT_WORDS: Record<SideTab, string> = {
  commands: '',
  log: 'unread',
  chat: 'unread',
  stack: 'on the stack',
}

/**
 * The count on a tab, drawn only when it is above zero: unread lines, or spells on the stack. Where the tab is named
 * by an `aria-label` that already has the count, the badge is `hidden` from assistive technology.
 */
function Count({ tab, className, hidden = false }: { tab: SideTab; className?: string; hidden?: boolean }) {
  const count = useSide().counts[tab]
  if (count === 0) return null
  return (
    <span
      data-slot="side-count"
      data-tab={tab}
      aria-hidden={hidden || undefined}
      className={cn(
        'inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-xs font-medium tabular-nums',
        tab === 'stack' ? 'bg-foreground text-background' : 'bg-primary text-on-primary',
        className,
      )}
    >
      {countLabel(count)}
      {/* The space is outside the span: a name is built from trimmed pieces, which would run "3" into "unread". */}
      {hidden ? null : ' '}
      {hidden ? null : <span className="sr-only">{COUNT_WORDS[tab]}</span>}
    </span>
  )
}

/** The tabs of the viewer: a seat has Commands first, a viewer that is no seat has none. */
function useTabs(): typeof TABS {
  const seated = usePlay((state) => state.seat !== null)
  return seated ? TABS : TABS.filter(({ tab }) => tab !== 'commands')
}

/** The side area's name for this viewer: what its tabs hold. */
function useSideName(): string {
  return usePlay((state) => state.seat !== null) ? SEATED_NAME : SPECTATOR_NAME
}

/** The panels; only the one on show is in the page. */
function Panels() {
  return (
    <>
      <TabsContent value="commands" className="h-full">
        <CommandsPanel />
      </TabsContent>
      <TabsContent value="log" className="h-full">
        <LogPanel />
      </TabsContent>
      <TabsContent value="chat" className="h-full">
        <ChatPanel />
      </TabsContent>
      <TabsContent value="stack" className="h-full overflow-y-auto">
        <StackPanel />
      </TabsContent>
    </>
  )
}

// A press on the tab bar goes through `activate`, so Base UI's own change of value is left out.
const noChange = (): void => undefined

/** Log, Chat and Stack as tabs with their counts, and the panel of the one on show. */
function TabbedPanels({ listClassName }: { listClassName?: string }) {
  const side = useSide()
  const tabs = useTabs()
  return (
    <Tabs value={side.tab} onValueChange={noChange} className="min-h-0 flex-1 gap-0">
      <TabsList className={cn('m-2 w-auto', listClassName)}>
        {tabs.map(({ tab, label }) => (
          <TabsTrigger
            key={tab}
            value={tab}
            className="min-w-0 flex-1 gap-1 px-1.5"
            onClick={() => side.activate(tab)}
          >
            {label}
            {/* Explicit space: a screen reader joins the pieces of a name, and an inline span adds none. */}{' '}
            <Count tab={tab} />
          </TabsTrigger>
        ))}
      </TabsList>
      <div className="min-h-0 flex-1 px-2 pb-2">
        <Panels />
      </div>
    </Tabs>
  )
}

/**
 * The rail of a medium window: an icon tab each on the table's edge, opening the panel over the table's boards (the
 * overlay of `SidePanelMount`), which leaves the hand and the controls dock free.
 */
function Rail() {
  const side = useSide()
  const tabs = useTabs()
  const name = useSideName()
  const close = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && side.open) side.setOpen(false)
  }
  return (
    <aside
      aria-label={name}
      data-side="rail"
      data-open={side.open || undefined}
      onKeyDown={close}
      className="relative flex min-h-0 flex-col items-center border-l bg-surface py-1"
    >
      <Tabs value={side.tab} onValueChange={noChange} orientation="vertical" className="min-h-0 flex-1 gap-0">
        <TabsList className="flex-col bg-transparent p-0">
          {tabs.map(({ tab, label, Icon }) => {
            const count = side.counts[tab]
            return (
              <TabsTrigger
                key={tab}
                value={tab}
                aria-label={count === 0 ? label : `${label}, ${countLabel(count)} ${COUNT_WORDS[tab]}`}
                title={label}
                className="relative size-11"
                onClick={() => side.activate(tab)}
              >
                <Icon aria-hidden="true" className="size-5" />
                <Count tab={tab} hidden className="absolute top-0.5 right-0.5 h-4 min-w-4 text-label" />
              </TabsTrigger>
            )
          })}
        </TabsList>
        {side.open && side.mount !== null
          ? createPortal(
              <div
                data-slot="rail-panel"
                className="pointer-events-auto absolute inset-y-0 right-0 w-80 border-l bg-surface p-2 shadow-e2"
              >
                <Panels />
              </div>,
              side.mount,
            )
          : null}
      </Tabs>
    </aside>
  )
}

/** The side column of a wide window: tabs on top, the panel under them, always up. */
function Column() {
  const name = useSideName()
  return (
    <aside aria-label={name} data-side="column" className="flex min-h-0 flex-col border-l bg-surface">
      <TabbedPanels />
    </aside>
  )
}

/** The side area as the grid draws it: a column or a rail; a sheet is drawn from the top bar instead (`SideSheet`). */
export function SideTabs() {
  const { kind } = useSide()
  if (kind === 'column') return <Column />
  return kind === 'rail' ? <Rail /> : null
}

/** The button of the top bar that opens the sheet, with what wants a look on it. */
export function SideSheetButton({ compact = false }: { compact?: boolean }) {
  const side = useSide()
  const name = useSideName()
  if (side.kind !== 'sheet') return null
  const waiting = side.attention
  return (
    <Button
      variant="ghost"
      size={compact ? 'icon-sm' : 'icon'}
      className="relative"
      aria-haspopup="dialog"
      aria-expanded={side.open}
      aria-label={waiting === 0 ? name : `${name}, ${countLabel(waiting)} new`}
      onClick={() => side.setOpen(!side.open)}
    >
      <PanelRightIcon aria-hidden="true" />
      {waiting === 0 ? null : (
        <span
          aria-hidden="true"
          data-slot="side-count"
          className="absolute top-0.5 right-0.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-primary px-1 text-label font-medium text-on-primary tabular-nums"
        >
          {countLabel(waiting)}
        </span>
      )}
    </Button>
  )
}

/**
 * The overlay of the table's main area that an open rail panel or sheet is drawn into: the rows from the opponents down
 * to `rows` (`sidePanelRows`), so the top bar, the hand and the controls dock are never under it. It lets every press
 * through but the panel's own. A column needs none.
 */
export function SidePanelMount({ rows }: { rows: string }) {
  const side = useSide()
  if (side.kind === 'column') return null
  return (
    <div
      ref={(element) => side.setMount(element)}
      data-slot="side-overlay"
      style={{ gridRow: rows, gridColumn: '1 / -1' } as CSSProperties}
      className="pointer-events-none relative z-40 min-h-0 min-w-0"
    />
  )
}

/** The sheet of a narrow window: it hangs from the top bar over the opponents' area and holds the same tabs. */
export function SideSheet() {
  const side = useSide()
  const name = useSideName()
  if (side.kind !== 'sheet' || side.mount === null) return null
  return (
    <Sheet open={side.open} onOpenChange={side.setOpen}>
      <SheetContent container={side.mount} data-side="sheet">
        <SheetTitle>{name}</SheetTitle>
        {/* The close button sits at the right end of the tab bar. */}
        <TabbedPanels listClassName="mr-12" />
      </SheetContent>
    </Sheet>
  )
}

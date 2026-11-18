import { useCallback, useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { ChatLine } from '@/play/store'
import type { SideKind } from './layout'
import { usePickStore } from './manual/panel/pick'
import { hasDetail } from './prompt/detail/asking'
import { ownDecision } from './prompt/ownDecision'
import { SideContext } from './sideContext'
import type { SideState } from './sideContext'
import { unreadChat } from './sideCounts'
import type { SideTab } from './sideCounts'

/** How long the Stack tab stays forward after a decision is answered, in ms: the next question of a run arrives within it. */
export const DETAIL_HOLD_MS = 700

/** Where the reader had got to in the chat. */
interface Seen {
  chat: ChatLine | null
}

/**
 * Holds the side area's state for the top bar and the side tabs: which tab is on show, whether its panel is up and
 * what has been written since it was last on show. It sits above both, so the table around them does not redraw
 * when a count changes.
 */
export function SideProvider({
  kind,
  foldable = false,
  children,
}: {
  kind: SideKind
  /** The window has room for a column (`Regions.foldable`). */
  foldable?: boolean
  children: ReactNode
}) {
  const { session } = useGame()
  // Until a tab is chosen the side shows the log, what a player reads during play; Commands is one press away, and
  // comes forward by itself when a card is waited for or work by hand comes due.
  const [chosen, setTab] = useState<SideTab | null>(null)
  const tab: SideTab = chosen ?? 'log'
  const due = usePlay((state) => state.seat !== null && state.dueLines.length > 0)
  const [wasDue, setWasDue] = useState(due)
  if (due !== wasDue) {
    setWasDue(due)
    if (due) setTab('commands')
  }
  // A decision with detail brings the Stack tab forward and gives the tab back afterwards, unless the viewer chose
  // another meanwhile. The gap between two questions of a run is bridged, so the column does not flip back and forth.
  const asked = usePlay((state) => hasDetail(ownDecision(state)))
  const [lately, setLately] = useState(asked)
  useEffect(() => {
    if (asked) {
      setLately(true)
      return undefined
    }
    const timer = setTimeout(() => setLately(false), DETAIL_HOLD_MS)
    return () => clearTimeout(timer)
  }, [asked])
  const [sheetFolded, setSheetFolded] = useState<boolean | null>(null)
  const deciding = asked || lately
  const [wasDeciding, setWasDeciding] = useState(false)
  const [before, setBefore] = useState<SideTab | null | undefined>(undefined)
  if (deciding !== wasDeciding) {
    setWasDeciding(deciding)
    if (deciding) {
      setBefore(chosen)
      setTab('stack')
    } else {
      if (before !== undefined && chosen === 'stack') setTab(before)
      setBefore(undefined)
      // The next decision's sheet starts as its kind asks, whatever the viewer did with this one.
      setSheetFolded(null)
    }
  }
  const [detail, setDetail] = useState<HTMLElement | null>(null)
  const [sheetStart, setSheetStart] = useState(false)
  const [panel, setPanel] = useState(false)
  const [mount, setMount] = useState<HTMLElement | null>(null)
  const [seen, setSeen] = useState<Seen>(() => {
    const { chat } = session.store.getState()
    return { chat: chat.at(-1) ?? null }
  })
  const open = kind === 'column' || panel
  // A card is waited for: a panel that covers the table steps aside, and returns with the card chosen or the wait over.
  const picks = usePickStore()
  useEffect(
    () =>
      picks?.watch((up) => {
        if (up) {
          setTab('commands')
          setPanel(true)
        } else if (kind !== 'column') setPanel(false)
      }),
    [picks, kind],
  )
  const chat = usePlay((state) => unreadChat(state.chat, seen.chat, state.seat))
  const stack = usePlay((state) => state.view?.stack.count ?? 0)

  // What was on show when the reader looks away has been read: everything up to that moment.
  useEffect(() => {
    if (!open || tab === 'stack' || tab === 'commands') return
    return () => {
      const { chat: said } = session.store.getState()
      setSeen((was) => ({
        chat: tab === 'chat' ? (said.at(-1) ?? was.chat) : was.chat,
      }))
    }
  }, [open, tab, session])

  const activate = useCallback(
    (next: SideTab): void => {
      // The rail's icon that opened the panel shuts it; the tabs of a sheet are inside it, so a press there only chooses.
      if (kind === 'rail' && next === tab && panel) {
        setPanel(false)
        return
      }
      setTab(next)
      setPanel(true)
    },
    [kind, tab, panel],
  )

  const counts = useMemo<Record<SideTab, number>>(
    () => ({
      commands: 0,
      log: 0,
      chat: open && tab === 'chat' ? 0 : chat,
      stack,
    }),
    [open, tab, chat, stack],
  )
  const value = useMemo<SideState>(
    () => ({
      kind,
      foldable,
      tab,
      open,
      counts,
      attention: counts.chat + counts.stack,
      activate,
      setOpen: setPanel,
      mount,
      setMount,
      detail,
      setDetail,
      sheetFolded,
      setSheetFolded,
      sheetStart,
      setSheetStart,
    }),
    [kind, foldable, tab, open, counts, activate, mount, detail, sheetFolded, sheetStart],
  )
  return <SideContext value={value}>{children}</SideContext>
}

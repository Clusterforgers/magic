import { useCallback, useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { ChatLine } from '@/play/store'
import type { SideKind } from './layout'
import { usePickStore } from './manual/panel/pick'
import { SideContext } from './sideContext'
import type { SideState } from './sideContext'
import { unreadChat } from './sideCounts'
import type { SideTab } from './sideCounts'

/** Where the reader had got to in the chat. */
interface Seen {
  chat: ChatLine | null
}

/**
 * Holds the side area's state for the top bar and the side tabs: which tab is on show, whether its panel is up and
 * what has been written since it was last on show. It sits above both, so the table around them does not redraw
 * when a count changes.
 */
export function SideProvider({ kind, children }: { kind: SideKind; children: ReactNode }) {
  const { session } = useGame()
  // Until a tab is chosen a seat shows its Commands, and a viewer that is no seat, which has none, the log.
  const [chosen, setTab] = useState<SideTab | null>(null)
  const seated = usePlay((state) => state.seat !== null)
  const tab: SideTab = chosen ?? (seated ? 'commands' : 'log')
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
      tab,
      open,
      counts,
      attention: counts.chat + counts.stack,
      activate,
      setOpen: setPanel,
      mount,
      setMount,
    }),
    [kind, tab, open, counts, activate, mount],
  )
  return <SideContext value={value}>{children}</SideContext>
}

import { useEffect, useState } from 'react'
import { useStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { isYourTurn } from '@/features/table/selectors'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { useTabHidden } from '@/hooks/useTabHidden'
import type { PlayStore } from '@/play/store'

/** What goes before the page title during this seat's turn. */
export const YOUR_TURN_PREFIX = 'Your turn · '
/** The page title that calls a backgrounded tab back when this seat must decide. */
export const YOUR_MOVE_TITLE = '● Your move'
/** How long each of the two titles shows while the tab flashes. */
export const FLASH_MS = 1000

// A flag that flips every `ms` while `active`, and is back to true when it stops.
function useBlink(active: boolean, ms: number): boolean {
  const [on, setOn] = useState(true)
  useEffect(() => {
    if (!active) return undefined
    const timer = window.setInterval(() => setOn((value) => !value), ms)
    return () => {
      window.clearInterval(timer)
      setOn(true)
    }
  }, [active, ms])
  return on
}

/**
 * `title` for the tab of the game page: "Your turn · " in front on this seat's turn, and "● Your move" in its place
 * while the tab is hidden and a decision waits (flashing against the other, still under reduced motion). Unchanged
 * while `enabled` is false. Feed it to `usePageTitle`, which restores the old title when the page goes.
 */
export function useTurnTitle(store: PlayStore, title: string, enabled: boolean): string {
  const { yourTurn, deciding } = useStore(
    store,
    useShallow((state) => ({ yourTurn: isYourTurn(state), deciding: state.pending !== null })),
  )
  const hidden = useTabHidden()
  const reduced = useReducedMotion()
  const calling = enabled && deciding && hidden
  const blink = useBlink(calling && !reduced, FLASH_MS)
  const plain = enabled && yourTurn ? `${YOUR_TURN_PREFIX}${title}` : title
  return calling && blink ? YOUR_MOVE_TITLE : plain
}

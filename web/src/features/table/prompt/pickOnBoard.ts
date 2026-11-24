/**
 * Pick on board: a card choice whose cards stand in the centre overlay, some of them on the table too, may hide the
 * overlay so the table's own marked cards are chosen in place (two copies of one creature, told apart by where they
 * stand). The prompt's home owns the switch, so its Show choices answer stands beside Confirm while the overlay is away.
 */
import { createContext, useCallback, useContext, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { usePromptKeys } from './usePromptKeys'

/** What a prompt's home tells its card choice about Pick on board. */
export interface PickOnBoard {
  /** The overlay is hidden and the table takes the presses. */
  hidden: boolean
  /** Whether the choice can be picked on the table now; the switch and its key V exist only then. */
  offer(on: boolean): void
  /** Hides or shows the overlay. */
  flip(): void
}

/** The switch of the prompt around the caller; none outside a prompt's layout. */
export const PickOnBoardContext = createContext<PickOnBoard | null>(null)

/** The home's side of Pick on board: the context value and whether it is offered. */
export function usePickOnBoardHost(): { value: PickOnBoard; offered: boolean } {
  const [offered, setOffered] = useState(false)
  const [peek, setPeek] = useState(false)
  const hidden = offered && peek
  // The switch is a button in the overlay while the cards show and in the home while they hide: focus follows it.
  const refocus = useRef(false)
  const flip = useCallback(() => {
    refocus.current = (document.activeElement?.closest('[data-slot="pick-on-board"]') ?? null) !== null
    setPeek((was) => !was)
  }, [])
  useLayoutEffect(() => {
    if (!refocus.current) return
    refocus.current = false
    document.querySelector<HTMLElement>('[data-slot="pick-on-board"]')?.focus()
  }, [hidden])
  usePromptKeys({ v: flip }, offered)
  // A choice that stops offering it (answered, or its cards left the table) starts the next one with its cards shown.
  const offer = useCallback((on: boolean) => {
    setOffered(on)
    if (!on) setPeek(false)
  }, [])
  const value = useMemo(() => ({ hidden, offer, flip }), [hidden, offer, flip])
  return { value, offered }
}

/** A card choice's side: offers Pick on board while `on`, and reads whether the overlay is hidden. */
export function usePickOnBoard(on: boolean): PickOnBoard | null {
  const pick = useContext(PickOnBoardContext)
  const offer = pick?.offer
  useLayoutEffect(() => {
    if (offer === undefined || !on) return undefined
    offer(true)
    return () => offer(false)
  }, [offer, on])
  return pick
}

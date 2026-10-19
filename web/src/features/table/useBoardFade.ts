import { useEffect, useState } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import type { PlayerId } from '@/protocol/generated'
import { useCueOptions } from './turn/useTurnFeed'

/** How long the large board of focus mode takes to give way to the next; `focusBoard.css` repeats it and `focusBoardCss.test.ts` keeps the two equal. */
export const FOCUS_FADE_MS = 150

/** The boards the focus area holds: the large one and, while a change fades, the one that is leaving. */
export interface BoardFade {
  /** The seat whose board is large now. */
  shown: PlayerId
  /** The seat whose board fades out for `FOCUS_FADE_MS` after a change; null otherwise, and always without cues or under reduced motion. */
  leaving: PlayerId | null
}

/**
 * Follows the seat the focus area shows. A change keeps the old board in view for `FOCUS_FADE_MS`, so it can fade out
 * while the new one fades in; under reduced motion, or where the table draws no timed cues, the new board is simply
 * there. It reads only which seat is shown and never holds back what the table does.
 */
export function useBoardFade(seat: PlayerId): BoardFade {
  const { on, slow } = useCueOptions()
  const reduced = useReducedMotion()
  const fades = on && !reduced
  const [state, setState] = useState<BoardFade>({ shown: seat, leaving: null })
  let current = state
  if (state.shown !== seat) {
    // Set while rendering, so the old board is in the same frame as the new one and is never missing for one.
    current = { shown: seat, leaving: fades ? state.shown : null }
    setState(current)
  }
  const { leaving } = current
  useEffect(() => {
    if (leaving === null) return undefined
    const timer = setTimeout(
      () => setState((was) => (was.leaving === leaving ? { ...was, leaving: null } : was)),
      FOCUS_FADE_MS * slow,
    )
    return () => clearTimeout(timer)
  }, [leaving, slow])
  return current
}

/**
 * The line a press on a card that cannot be played shows above the card for a moment (`blockedTell.ts`): calm, one line,
 * gone after a few seconds, at the next press elsewhere or on Escape. It is spoken politely, so a screen reader hears it.
 */
import { useEffect } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { useSeatNamer } from './names'
import { useBlockedTell, waitingLine } from './blockedTell'
import { waitingOn, waitingText } from './turnText'

/** The gutter the line keeps from the window's sides, in px. */
const GUTTER = 16
/** The widest the line grows, in px. */
const WIDEST = 320
/** The space between the line and the card, in px. */
const GAP = 8
/** The room above a card the line needs to stand there, in px; a card higher up gets it below. */
const ROOM = 72

/** The tell on screen, placed above the pressed card and kept inside the window. */
export function BlockedTell() {
  const tell = useBlockedTell((state) => state.tell)
  const hush = useBlockedTell((state) => state.hush)
  const name = useSeatNamer()
  const waiting = usePlay((state) => (tell === null || tell.line !== undefined ? null : waitingText(waitingOn(state), name)))
  useEffect(() => {
    if (tell === null) return
    const away = (event: PointerEvent): void => {
      const target = event.target instanceof Element ? event.target : null
      if (target?.closest(`[data-object-id="${tell.card}"]`) === null) hush(tell.token)
    }
    const escape = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') hush(tell.token)
    }
    document.addEventListener('pointerdown', away, true)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('pointerdown', away, true)
      document.removeEventListener('keydown', escape)
    }
  }, [tell, hush])
  if (tell === null) return null
  const line = tell.line ?? (waiting === null ? null : waitingLine(waiting))
  if (line === null) return null
  const viewport = typeof window === 'undefined' ? 1024 : window.innerWidth
  const width = Math.min(WIDEST, viewport - 2 * GUTTER)
  const centre = tell.anchor.left + tell.anchor.width / 2
  const left = Math.min(Math.max(GUTTER, centre - width / 2), viewport - GUTTER - width)
  const tall = typeof window === 'undefined' ? 768 : window.innerHeight
  // Above the card, as a hand card is pressed at the foot of the screen; below one too near the top for the line.
  const above = tell.anchor.top >= ROOM
  const place = above
    ? { bottom: tall - tell.anchor.top + GAP }
    : { top: tell.anchor.top + tell.anchor.height + GAP }
  return (
    <div
      key={tell.token}
      role="status"
      aria-live="polite"
      data-slot="blocked-tell"
      data-card={tell.card}
      className={cn(
        'pointer-events-none fixed z-50 flex justify-center motion-safe:animate-in motion-safe:fade-in-0 motion-safe:zoom-in-95 motion-safe:duration-base',
        above ? 'items-end motion-safe:slide-in-from-bottom-1' : 'items-start motion-safe:slide-in-from-top-1',
      )}
      style={{ left, width, ...place }}
    >
      <p className="rounded-md bg-popover px-3 py-1.5 text-center text-sm leading-5 text-popover-foreground shadow-md ring-1 ring-foreground/15">
        {line}
      </p>
    </div>
  )
}

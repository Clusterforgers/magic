/**
 * The enlarged board of one opponent of a grid: a press on the enlarge button of its pod draws that seat alone over the
 * opponents' row at its own card size, so its cards read whole. Escape, the close button or a press outside the row
 * closes it; a decision other than priority closes it too, so no card a prompt asks for is ever out of sight.
 */
import { Maximize2Icon, XIcon } from 'lucide-react'
import { createContext, useContext, useEffect } from 'react'
import type { RefObject } from 'react'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { PlayState } from '@/play/store'
import type { PlayerId } from '@/protocol/generated'

/** The zoom of a grid: the seat enlarged, if any, and how to change it. */
export interface BoardZoom {
  zoomed: PlayerId | null
  setZoomed(seat: PlayerId | null): void
}

/** Provided by the opponents' grid; a pod outside it has no enlarge button. */
export const BoardZoomContext = createContext<BoardZoom | null>(null)

/** The pending decision's key when it is no priority (a prompt that may ask for any card), else null; wrap nothing. */
export function promptKey(state: PlayState): string | null {
  const pending = state.pending
  if (pending === null) return null
  const kind = pending.decision.kind
  return typeof kind === 'object' && 'Priority' in kind ? null : String(pending.id)
}

/** The enlarge button of a seat's pod: opens its board enlarged, or closes it while it is. */
export function ZoomButton({ seat, owner }: { seat: PlayerId; owner: string }) {
  const zoom = useContext(BoardZoomContext)
  if (zoom === null) return null
  const open = zoom.zoomed === seat
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      data-slot="board-zoom-toggle"
      aria-pressed={open}
      aria-label={open ? `Back to every board` : `Enlarge ${owner}'s board`}
      title={open ? 'Back to every board (Esc)' : `Enlarge ${owner}'s board`}
      onClick={(event) => {
        // A pod is a defender target while an attack is aimed; this press is the button's.
        event.stopPropagation()
        zoom.setZoomed(open ? null : seat)
      }}
    >
      {open ? <XIcon aria-hidden="true" /> : <Maximize2Icon aria-hidden="true" />}
    </Button>
  )
}

/**
 * Closes the zoom on Escape, on a press outside `area`, and when a decision other than priority arrives; while
 * nothing is zoomed it listens to nothing.
 */
export function useZoomClose(zoom: BoardZoom, area: RefObject<HTMLElement | null>): void {
  const { zoomed, setZoomed } = zoom
  const prompt = usePlay(promptKey)
  useEffect(() => {
    if (zoomed !== null && prompt !== null) setZoomed(null)
  }, [prompt, zoomed, setZoomed])
  useEffect(() => {
    if (zoomed === null) return undefined
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setZoomed(null)
    }
    const onPointer = (event: PointerEvent) => {
      const target = event.target
      if (target instanceof Node && area.current !== null && !area.current.contains(target)) setZoomed(null)
    }
    window.addEventListener('keydown', onKey)
    document.addEventListener('pointerdown', onPointer, true)
    return () => {
      window.removeEventListener('keydown', onKey)
      document.removeEventListener('pointerdown', onPointer, true)
    }
  }, [zoomed, setZoomed, area])
}

import { useEffect } from 'react'
import type { FocusEvent, MouseEvent, PointerEvent } from 'react'
import type { ObjectId } from '@/protocol/generated'
import { anchorOf, usePreviewStore } from './preview'

/** The events that drive the preview of one card; spread them on its element. */
export interface PreviewHandlers {
  onClick(event: MouseEvent<HTMLElement>): void
  onPointerEnter(event: PointerEvent<HTMLElement>): void
  onPointerLeave(): void
  onFocus(event: FocusEvent<HTMLElement>): void
  onBlur(): void
}

/**
 * Hover (mouse only) and keyboard focus show the card's preview, leaving hides it, a click pins it. A click the caller
 * handles (`onClick`: the card plays, taps or is chosen) pins nothing: it is an act and not a request to look, and a
 * pinned preview covers a part of the table (the prompt among it) until it is closed.
 */
export function usePreviewHandlers(id: ObjectId, onClick?: (card: HTMLElement) => void): PreviewHandlers {
  const preview = usePreviewStore()
  // An element that leaves the screen while hovered gets no pointer-leave.
  useEffect(() => () => preview.hide(id), [preview, id])
  return {
    onClick(event) {
      if (onClick !== undefined) {
        onClick(event.currentTarget)
        return
      }
      preview.pin(id, anchorOf(event.currentTarget))
    },
    onPointerEnter(event) {
      if (event.pointerType === 'mouse') preview.show(id, anchorOf(event.currentTarget))
    },
    onPointerLeave: () => preview.hide(id),
    onFocus: (event) => preview.show(id, anchorOf(event.currentTarget)),
    onBlur: () => preview.hide(id),
  }
}

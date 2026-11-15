import { useEffect } from 'react'
import type { RefObject } from 'react'
import type { PreviewStore, PreviewTarget } from './preview'

/**
 * Closes a pinned preview on a press outside its panel and outside the card it shows, mouse or touch. The press is not
 * stopped, so it still reaches what it lands on: another card then pins its own preview with its click. The card
 * that opened the preview is left to its own second click, which closes it, so the press does not reopen it. A
 * preview that is only hovered listens to nothing.
 */
export function usePreviewDismiss(
  target: PreviewTarget,
  panel: RefObject<HTMLElement | null>,
  store: PreviewStore,
): void {
  const { id, pinned } = target
  useEffect(() => {
    if (!pinned) return undefined
    const away = (event: PointerEvent): void => {
      const hit = event.target
      if (!(hit instanceof Element) || panel.current?.contains(hit) === true) return
      if (hit.closest(`[data-object-id="${id}"]`) !== null) return
      store.dismiss()
    }
    document.addEventListener('pointerdown', away, true)
    return () => document.removeEventListener('pointerdown', away, true)
  }, [id, pinned, panel, store])
}

import { useEffect } from 'react'
import type { RefObject } from 'react'

/**
 * Scrolls the sideways strip around `item` just far enough to show it while `active` is true. Only the strip
 * moves, never the page, so a turn that passes while the reader is further down the table does not pull them up.
 * `item` sits in a slot (its parent) that is a child of the strip, and the strip must be positioned (`relative`)
 * for the slot's offsets to be read from it.
 */
export function useKeepInView(item: RefObject<HTMLElement | null>, active: boolean): void {
  useEffect(() => {
    const slot = item.current?.parentElement
    const strip = slot?.parentElement
    if (!active || slot == null || strip == null) return
    const { offsetLeft, offsetWidth } = slot
    if (offsetLeft < strip.scrollLeft) strip.scrollTo({ left: offsetLeft })
    else if (offsetLeft + offsetWidth > strip.scrollLeft + strip.clientWidth) {
      strip.scrollTo({ left: offsetLeft + offsetWidth - strip.clientWidth })
    }
  }, [item, active])
}

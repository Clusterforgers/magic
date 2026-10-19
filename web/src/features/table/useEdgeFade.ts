import { useLayoutEffect } from 'react'
import type { RefObject } from 'react'

/** Which edges of a scrolling zone hide more: none, the bottom (at the top of the zone), the top, or both. */
export type Fade = 'none' | 'top' | 'bottom' | 'both'

/** The fade for a zone with these scroll metrics in px; one px of slack covers fractional scroll positions. */
export function fadeOf(scrollTop: number, clientHeight: number, scrollHeight: number): Fade {
  const room = scrollHeight - clientHeight
  if (room <= 1) return 'none'
  if (scrollTop <= 1) return 'bottom'
  return scrollTop >= room - 1 ? 'top' : 'both'
}

/**
 * Keeps `data-fade` of a scrolling zone (the `fade-scroll` utility of `index.css`) equal to what is hidden above and
 * below, so a zone that cannot show everything fades the edge that hides more instead of ending in a hard cut.
 */
export function useEdgeFade(ref: RefObject<HTMLElement | null>): void {
  useLayoutEffect(() => {
    const zone = ref.current
    if (zone === null) return
    const update = (): void => {
      const next = fadeOf(zone.scrollTop, zone.clientHeight, zone.scrollHeight)
      if (zone.dataset['fade'] !== next) zone.dataset['fade'] = next
    }
    update()
    zone.addEventListener('scroll', update, { passive: true })
    // What the zone holds changes the scroll height without changing the zone's own box.
    const sizes = typeof ResizeObserver === 'function' ? new ResizeObserver(update) : null
    const watchChildren = (): void => {
      sizes?.disconnect()
      sizes?.observe(zone)
      for (const child of zone.children) sizes?.observe(child)
      update()
    }
    watchChildren()
    const children = typeof MutationObserver === 'function' ? new MutationObserver(watchChildren) : null
    children?.observe(zone, { childList: true })
    return () => {
      zone.removeEventListener('scroll', update)
      sizes?.disconnect()
      children?.disconnect()
    }
  }, [ref])
}

// What the main navigation's row has room for: the tabs that fit beside a More menu, and which tab is the current page.

/**
 * How many tabs, from the first, the row shows in `room` px: all of them where they fit, else as many as fit beside
 * the More button (`more` px), which takes the rest. Never fewer than none.
 */
export function tabsThatFit(widths: readonly number[], more: number, room: number): number {
  // Half a pixel of slack: widths are fractional and the row's own width is rounded.
  const slack = 0.5
  const total = widths.reduce((sum, width) => sum + width, 0)
  if (total <= room + slack) return widths.length
  let used = more
  let shown = 0
  for (const width of widths) {
    if (used + width > room + slack) break
    used += width
    shown += 1
  }
  return shown
}

/** Whether `pathname` is the page of a tab at `to`: that address, or one under it unless the tab wants it `exact`. */
export function isCurrentTab(to: string, pathname: string, exact = false): boolean {
  if (pathname === to) return true
  if (exact || to === '/') return false
  return pathname.startsWith(`${to}/`)
}

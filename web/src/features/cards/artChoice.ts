/** What the art picker shows of a printing, and how it filters them; pure, so the picker stays a view. */
import type { PrintingView } from '@/protocol/generated'

/** The printings whose set code or set name holds `query` (any case); every printing for a blank query. */
export function filterPrintings(printings: readonly PrintingView[], query: string): PrintingView[] {
  const needle = query.trim().toLowerCase()
  if (needle === '') return [...printings]
  return printings.filter(
    (printing) =>
      printing.set.toLowerCase().includes(needle) || (printing.set_name ?? '').toLowerCase().includes(needle),
  )
}

/** The short line under a printing: set code and collector number, then the year. */
export function printingCaption(printing: PrintingView): string {
  const year = printing.released_at?.slice(0, 4)
  const head = `${printing.set.toUpperCase()} #${printing.collector_number}`
  return year === undefined ? head : `${head} · ${year}`
}

/** The words that tell a printing's look apart: full art, the finishes beyond plain, a frame before the modern one. */
export function printingTraits(printing: PrintingView): string[] {
  const traits: string[] = []
  if (printing.full_art) traits.push('Full art')
  for (const finish of printing.finishes) {
    if (finish === 'foil') traits.push('Foil')
    if (finish === 'etched') traits.push('Etched')
  }
  if (printing.frame === '1993' || printing.frame === '1997') traits.push('Old frame')
  if (printing.frame === 'future') traits.push('Future frame')
  if (printing.digital) traits.push('Digital')
  if (printing.lang !== 'en') traits.push(printing.lang.toUpperCase())
  return traits
}

/** The accessible name of a printing's option: set name (or code), number, year and traits. */
export function printingLabel(printing: PrintingView): string {
  const set = printing.set_name ?? printing.set.toUpperCase()
  const year = printing.released_at?.slice(0, 4)
  const parts = [`${set} #${printing.collector_number}`]
  if (year !== undefined) parts.push(year)
  parts.push(...printingTraits(printing))
  return parts.join(', ')
}

/** The option a grid key moves focus to from `at`, for a grid of `columns` and `count` options; null to stay. */
export function gridMove(key: string, at: number, columns: number, count: number): number | null {
  const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: columns, ArrowUp: -columns }
  if (key === 'Home') return 0
  if (key === 'End') return count - 1
  const delta = step[key]
  if (delta === undefined) return null
  const next = at + delta
  return next < 0 || next >= count ? null : next
}

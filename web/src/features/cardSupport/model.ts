/**
 * The words and numbers of the Card support page, as pure functions: the share line, the feed's lines, the words of the
 * newest daily check, the filter of the missing cards and the points of the trend.
 */
import { timeSince } from '@/lib/timeSince'
import type {
  FeedEvent,
  MissingCardView,
  MissingState,
  RefreshCheck,
  SupportPoint,
} from '@/protocol/generated'

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

/** "22 Nov" of a UTC time; the text itself when it is no time. */
export function shortDate(at: string): string {
  const time = new Date(Date.parse(at))
  if (Number.isNaN(time.getTime())) return at
  return `${time.getUTCDate()} ${MONTHS[time.getUTCMonth()] ?? ''}`
}

/** A share as a percentage with one decimal ("94.7%"); 0% of nothing. */
export function percent(part: number, whole: number): string {
  if (whole <= 0) return '0%'
  return `${((100 * part) / whole).toFixed(1)}%`
}

const NUMBER = new Intl.NumberFormat('en-US')

/** A count with thousands separators ("32,116"). */
export const count = (n: number): string => NUMBER.format(n)

const cards = (n: number): string => `${count(n)} ${n === 1 ? 'card' : 'cards'}`

/** Up to `shown` names joined with commas and "and N more" for the rest of `total`. */
export function someNames(names: readonly string[], total: number, shown = 5): string {
  const listed = names.slice(0, shown)
  const rest = total - listed.length
  if (listed.length === 0) return ''
  return rest > 0 ? `${listed.join(', ')} and ${count(rest)} more` : listed.join(', ')
}

/** One line of the feed: a title and the detail under it. */
export interface FeedLine {
  /** What happened, in one line. */
  title: string
  /** Names or reasons, possibly empty. */
  detail: string
  /** When, as the server wrote it. */
  at: string
  /** The kind, for the icon. */
  kind: FeedEvent['kind']
}

/** The line of one event. */
export function feedLine(event: FeedEvent): FeedLine {
  switch (event.kind) {
    case 'new_precons': {
      const decks = event.decks.length
      return {
        kind: event.kind,
        at: event.at,
        title: `${event.set_name ?? event.code}: ${decks} ${decks === 1 ? 'precon' : 'precons'} added`,
        detail: event.decks.join(', '),
      }
    }
    case 'new_cards':
      return {
        kind: event.kind,
        at: event.at,
        title: `${count(event.count)} new ${event.count === 1 ? 'card' : 'cards'}`,
        detail: someNames(event.names, event.count),
      }
    case 'now_automatic': {
      const gained = event.count > 0 ? `${cards(event.count)} now automatic` : null
      const release = event.release !== null && event.new_release ? event.release : null
      const title =
        release !== null && gained !== null
          ? `${release}: ${gained}`
          : release !== null
            ? `${release} released`
            : (gained ?? 'Card support changed')
      return { kind: event.kind, at: event.at, title, detail: someNames(event.names, event.count) }
    }
    case 'check_failed':
      return {
        kind: event.kind,
        at: event.at,
        title: 'The daily check failed',
        detail: event.failures.join('; '),
      }
  }
}

/** The words of the newest daily check, for the line under the title; null before the first. */
export function checkWords(check: RefreshCheck | null, now: number): string | null {
  if (check === null) return null
  const when = timeSince(check.finished_at ?? check.started_at, now)
  switch (check.state) {
    case 'running':
      return 'Checking for new cards and precons now…'
    case 'succeeded':
      return `Checked for new cards and precons ${when}.`
    case 'deferred':
      return `Checked ${when}; new cards load once no game is running.`
    case 'failed':
      return `The last check failed ${when}; the numbers are from the check before.`
  }
}

/** A filter of the missing cards. */
export interface MissingFilter {
  /** Words of the name or the blocker; empty keeps every card. */
  query: string
  /** One state, or every state. */
  state: MissingState | 'all'
}

/** Text without case or accents, so "Æther" and "aether" match. */
export function fold(text: string): string {
  return text.normalize('NFKD').replace(/\p{M}/gu, '').replace(/æ/giu, 'ae').toLowerCase()
}

/** The cards that match `filter`, in their order: every word of the query in the name or the blocker. */
export function filterMissing(cards: readonly MissingCardView[], filter: MissingFilter): MissingCardView[] {
  const words = fold(filter.query).split(/\s+/).filter(Boolean)
  return cards.filter((card) => {
    if (filter.state !== 'all' && card.state !== filter.state) return false
    if (words.length === 0) return true
    const text = fold(`${card.name} ${card.blocker} ${card.detail}`)
    return words.every((word) => text.includes(word))
  })
}

/** How many of `cards` are in each state. */
export function stateCounts(cards: readonly MissingCardView[]): Record<MissingState, number> {
  const counts: Record<MissingState, number> = { waiting: 0, partial: 0, manual: 0, other: 0 }
  for (const card of cards) counts[card.state] += 1
  return counts
}

/** A point of the trend in the chart's box: x by time, y by share. */
export interface TrendPoint {
  /** 0 to `width`. */
  x: number
  /** 0 (top) to `height`. */
  y: number
  /** The share, 0 to 1. */
  share: number
  /** The snapshot it stands for. */
  point: SupportPoint
}

/**
 * The trend's points in a `width` by `height` box: time runs left to right (one point at the middle), and the share
 * runs from `floor` at the bottom to 100% at the top, where `floor` is the lowest share rounded down to 5% below it.
 */
export function trendPoints(
  history: readonly SupportPoint[],
  width: number,
  height: number,
): { points: TrendPoint[]; floor: number } {
  const shares = history.map((p) => (p.cards > 0 ? p.automatic / p.cards : 0))
  const lowest = shares.length === 0 ? 0 : Math.min(...shares)
  const floor = Math.max(0, Math.floor((lowest - 0.01) * 20) / 20)
  const times = history.map((p) => Date.parse(p.taken_at))
  const first = times[0] ?? 0
  const span = (times[times.length - 1] ?? 0) - first
  const points = history.map((point, at) => {
    const share = shares[at] ?? 0
    const time = times[at] ?? first
    const x = span > 0 ? ((time - first) / span) * width : width / 2
    const y = height - ((share - floor) / (1 - floor || 1)) * height
    return { x, y, share, point }
  })
  return { points, floor }
}

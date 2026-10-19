/**
 * The turn feed: what one batch tells the turn cues (the banner, the ticker and the ladder), worked out from the game
 * before the batch, the game after it and the batch's events alone. It reads nothing a seat does not already see on
 * its own screen and never says how another seat passed: a step counts as skipped when it began and ended inside one
 * batch, which the server makes the same for every seat (`docs/arena-flow.md`, Information leaks), and the only reason
 * it gives is for the viewer's own main phase.
 */
import type { PlayState } from '@/play/store'
import type { Event, EventView, HiddenEvent, PlayerId, Step } from '@/protocol/generated'
import { describeEvent } from '../eventText'
import type { EventNames } from '../eventText'
import { batchBetween } from '../fx/batch'
import type { Moment } from '../fx/derive'
import { STEP_LABEL } from '../turnText'
import { TICKER_MAX_LINES } from './timing'

/** Turns a seat number into the name the cues show. */
export type SeatNames = (seat: PlayerId) => string

/** What a ticker line is: a note on skipped steps, the reason for the viewer's own skip, or the newest event. */
export type TickerKind = 'skipped' | 'reason' | 'event'

/** One line of the ticker. */
export interface TickerLine {
  kind: TickerKind
  text: string
}

/** The words of the turn banner: whose turn has begun and which turn it is. */
export interface BannerCue {
  /** The viewer's own turn. */
  yours: boolean
  /** "Your turn" or "Bo's turn". */
  who: string
  /** The turn number. */
  turn: number
}

/** What one batch means for the turn cues. */
export interface Feed {
  /** Set when the turn number or the active seat changed over the batch. */
  banner: BannerCue | null
  /** The ticker lines of the batch, in the order they show, at most `TICKER_MAX_LINES`. */
  lines: readonly TickerLine[]
  /** The steps of the turn the batch ends in that went by without a window, in turn order, for the ladder. */
  skipped: readonly Step[]
  /** The sentence a polite live region says for the banner; empty when there is no banner. */
  announcement: string
}

/** Settings that depend on the viewer's own session rather than the batch. */
export interface FeedOptions {
  /** The viewer was passing on its own request (a yield), which explains its passes better than the table does. */
  yielding?: boolean
}

/** The most step names a skip note lists before it counts the rest. */
const NOTE_NAMES = 3

/** "Skipped Upkeep, Draw", or "Skipped Upkeep, Draw, Precombat main and 4 more" for a long run. */
export function skipNote(steps: readonly Step[]): string {
  const names = [...new Set(steps.map((step) => STEP_LABEL[step]))]
  if (names.length <= NOTE_NAMES) return `Skipped ${names.join(', ')}`
  return `Skipped ${names.slice(0, NOTE_NAMES).join(', ')} and ${names.length - NOTE_NAMES} more`
}

/** The only sentence that explains a skip, and only the viewer's own: the game passed its main phase for it. */
export const OWN_REASON = 'You had no plays'

interface Run {
  step: Step
  /** Somebody was given priority in the step, so it had a window that nobody was asked about. */
  granted: boolean
  /** The viewer is the active seat of the turn the step belongs to. */
  own: boolean
  /** How many turns had begun in the batch before the step did. */
  turns: number
}

// The steps begun in the batch, in order. Untap and cleanup never grant priority, so they never count as skipped.
function runsIn(events: readonly EventView[], viewer: PlayerId, active: PlayerId): Run[] {
  const runs: Run[] = []
  let turns = 0
  let seatActive = active
  for (const view of events) {
    if (!('Public' in view) || typeof view.Public === 'string') continue
    const event = view.Public
    if ('TurnBegan' in event) {
      seatActive = event.TurnBegan.active
      turns += 1
    } else if ('StepBegan' in event) {
      runs.push({ step: event.StepBegan.step, granted: false, own: seatActive === viewer, turns })
    } else if ('PriorityGranted' in event) {
      const last = runs.at(-1)
      if (last !== undefined) last.granted = true
    }
  }
  return runs
}

// What a seat is told of an event that moves a card in secret, without the card: its draw, and a move into a hand or library.
function withoutCard(event: Event): HiddenEvent | null {
  if (typeof event === 'string') return null
  if ('CardDrawn' in event) return { CardDrawn: { player: event.CardDrawn.player } }
  if ('ZoneChanged' in event) {
    const { from, to, cause } = event.ZoneChanged
    const inSecret = typeof to !== 'string' && ('Hand' in to || 'Library' in to)
    return inSecret ? { ZoneChanged: { from, to, cause } } : null
  }
  return null
}

// A viewer is shown its own draw as a draw, never as the card, so the line is the one every other seat reads.
function sameForEverySeat(view: EventView): EventView {
  if (!('Public' in view)) return view
  const hidden = withoutCard(view.Public)
  return hidden === null ? view : { Hidden: hidden }
}

const startsTurn = (view: EventView): boolean =>
  'Public' in view && typeof view.Public !== 'string' && 'TurnBegan' in view.Public

// The newest event line worth reading; the start of a turn is the banner's to tell.
function newestLine(events: readonly EventView[], names: EventNames, banner: boolean): TickerLine | null {
  for (const view of events.toReversed()) {
    const line = describeEvent(sameForEverySeat(view), names)
    if (line.minor) continue
    return banner && startsTurn(view) ? null : { kind: 'event', text: line.text }
  }
  return null
}

/**
 * What the batch that took the game from `before` to `after` means for the turn cues. Pure: the same game and events
 * give the same feed, and nothing else (no clock, no arrival time, no decision) goes in.
 */
export function deriveFeed(
  before: Moment,
  after: Moment,
  events: readonly EventView[],
  names: SeatNames,
  options: FeedOptions = {},
): Feed {
  const { viewer, stage, result } = after.view
  const was = before.view.turn
  const now = after.view.turn

  const changed = was.number !== now.number || was.active !== now.active
  const yours = now.active === viewer
  const banner: BannerCue | null =
    changed && stage === 'Turns' && result === null
      ? { yours, who: yours ? 'Your turn' : `${names(now.active)}'s turn`, turn: now.number }
      : null

  const runs = runsIn(events, viewer, was.active)
  // The last step begun is the one the batch ends in; every step before it began and ended inside the batch.
  const ended = runs.slice(0, -1).filter((run) => run.granted)
  const turnsBegun = runs.at(-1)?.turns ?? 0
  const skipped = ended.filter((run) => run.turns === turnsBegun).map((run) => run.step)

  const note = ended.length === 0 ? null : skipNote(ended.map((run) => run.step))
  const explained =
    options.yielding !== true &&
    ended.some((run) => run.own && (run.step === 'Main1' || run.step === 'Main2'))
  const eventNames: EventNames = {
    seat: names,
    card: (id) => after.objectsById[id]?.name ?? before.objectsById[id]?.name ?? 'a card',
  }
  const newest = newestLine(events, eventNames, banner !== null)
  const lines = [
    ...(note === null ? [] : [{ kind: 'skipped', text: note } satisfies TickerLine]),
    ...(explained ? [{ kind: 'reason', text: OWN_REASON } satisfies TickerLine] : []),
    ...(newest === null ? [] : [newest]),
  ].slice(0, TICKER_MAX_LINES)

  const announcement =
    banner === null ? '' : `${banner.who}, turn ${banner.turn}${note === null ? '' : `. ${note}`}`
  return { banner, lines, skipped, announcement }
}

const isEmpty = (feed: Feed): boolean =>
  feed.banner === null && feed.lines.length === 0 && feed.skipped.length === 0

// Each of the cues asks about the same batch, so the answer is kept for the state it was worked out for.
const worked = new WeakMap<PlayState, { prev: PlayState; feed: Feed | null }>()

/**
 * The feed of the change from `prev` to `next`, or null when it was not a batch (a snapshot, a decision, a chat line)
 * or the batch means nothing to the cues.
 */
export function feedBetween(prev: PlayState, next: PlayState, names: SeatNames): Feed | null {
  const hit = worked.get(next)
  if (hit !== undefined && hit.prev === prev) return hit.feed
  const step = batchBetween(prev, next)
  let feed: Feed | null = null
  if (step !== null) {
    const derived = deriveFeed(step.before, step.after, step.events, names, {
      yielding: prev.yielding !== null || next.yielding !== null,
    })
    feed = isEmpty(derived) ? null : derived
  }
  worked.set(next, { prev, feed })
  return feed
}

/** The prompts that lay out cards in the shared sheet, which then grows over the opponents' boards (`Regions.sheetWide`). */
import type { PlayState } from '@/play/store'

/** The decision kinds whose prompt draws a row of cards to look at or pick from, or a card's picture beside its modes. */
export const CARD_PROMPTS: ReadonlySet<string> = new Set([
  'ChooseCards',
  'ChooseTargets',
  'Scry',
  'OrderCards',
  'Mulligan',
  'ChooseMode',
])

/** Whether the viewer's pending decision lays out cards; wrap nothing, it is a boolean. */
export function laysOutCards(state: PlayState): boolean {
  const kind = state.pending?.decision.kind
  if (kind === undefined || typeof kind !== 'object') return false
  return Object.keys(kind).some((name) => CARD_PROMPTS.has(name))
}

/**
 * What the band takes of the sheet besides a choice's line of cards, in rem (`CardChoices`' `--choice-room`): the first
 * line (the status, the sentence and the answers) with the turn ticker under the band, a row of quick buttons above the
 * cards, a search box above them.
 */
export const BAND_HEAD_REM = 3.5
export const QUICK_ROW_REM = 2.5
export const SEARCH_ROW_REM = 2.75

/** From this many cards a choice in the band may wrap into two rows, where they are larger so than in one. */
export const STRIP_WRAP_FROM = 9

/** A card in the band's row: at most 7 rem and 11 % of the window's height wide, at least 3 rem. */
const CARD_MAX_REM = 7
const CARD_DVH = 11
const CARD_FLOOR_REM = 3
/** The row's padding on each side and the gap between its cards, in rem (`CardChoices`' `card-row`). */
const ROW_PAD_REM = 0.375
const GAP_REM = 0.5
/** What a row's height keeps besides its cards, in rem: its padding and a margin. */
const ROW_KEEP_REM = 2
/** Taken off a card's share of the width, in px, so rounding never pushes the last card of a row onto the next. */
const ROUNDING_PX = 0.5

/** The rows a choice of `count` cards in the band may take: one, or from `STRIP_WRAP_FROM` cards also two. */
export function stripRows(count: number): number[] {
  return count >= STRIP_WRAP_FROM ? [1, 2] : [1]
}

/**
 * The CSS width of each of `count` cards in the band's row (`--choice-fit`): for each of `stripRows`, the largest card
 * whose rows fit the room's height (`--choice-room`) and whose row fits the row's width (`cqi`, the row is the cards'
 * container), and of those the larger. The row wraps by itself, so no card is cut or scrolled sideways away.
 */
export function stripCardWidth(count: number): string {
  const options = stripRows(count).map((rows) => {
    const per = Math.max(1, Math.ceil(count / rows))
    const high = `calc((var(--choice-room, 20rem) - ${ROW_KEEP_REM}rem - ${(rows - 1) * GAP_REM}rem) / ${rows} * 5 / 7)`
    // `cqi` is the row's content box, inside its padding.
    const wide = `calc((100cqi - ${(per - 1) * GAP_REM}rem) / ${per} - ${ROUNDING_PX}px)`
    return `min(${CARD_MAX_REM}rem, ${CARD_DVH}dvh, ${high}, ${wide})`
  })
  const best = options.length === 1 ? (options[0] ?? '') : `max(${options.join(', ')})`
  return `max(${CARD_FLOOR_REM}rem, ${best})`
}

/** The room a choice of cards has in the band, in px: the height left for its rows, the row's width, the window's height. */
export interface StripRoom {
  room: number
  width: number
  windowHeight: number
}

/** What `stripCardWidth` gives in px for this room, with the rows the cards then take; for the tests. */
export function stripFit(
  count: number,
  { room, width, windowHeight }: StripRoom,
  rem = 16,
): { card: number; rows: number } {
  let best = { card: 0, rows: 1 }
  for (const rows of stripRows(count)) {
    const per = Math.max(1, Math.ceil(count / rows))
    const high = ((room - ROW_KEEP_REM * rem - (rows - 1) * GAP_REM * rem) / rows) * (5 / 7)
    const wide = (width - 2 * ROW_PAD_REM * rem - (per - 1) * GAP_REM * rem) / per - ROUNDING_PX
    const card = Math.min(CARD_MAX_REM * rem, (CARD_DVH * windowHeight) / 100, high, wide)
    if (card > best.card) best = { card, rows }
  }
  const card = Math.max(CARD_FLOOR_REM * rem, best.card)
  const per = Math.max(
    1,
    Math.floor((width - 2 * ROW_PAD_REM * rem + GAP_REM * rem) / (card + GAP_REM * rem)),
  )
  return { card, rows: Math.ceil(count / per) }
}

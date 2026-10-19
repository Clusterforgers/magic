/** Pure fit engine of the table: the largest card width at which a zone's rows of stacks wrap inside its box. */

/** A card is 5 wide and 7 high, so its height is this times its width. */
export const CARD_ASPECT = 7 / 5

/** Lands are drawn at this share of the width of the other cards. */
export const LAND_SCALE = 0.8

/** The room a zone has, in CSS px. */
export interface FitBox {
  width: number
  height: number
}

/** One row to place: how many stacks it holds and the shape of its cards. */
export interface FitGroup {
  /** The boxes in the row, one per stack. */
  stacks: number
  /** A card's height over its width; `CARD_ASPECT` when left out. */
  aspect?: number
  /** The row's card width as a share of the zone's card width; 1 when left out, `LAND_SCALE` for lands. */
  scale?: number
}

/** The widths the engine tries and the space between boxes, in CSS px. */
export interface FitLimits {
  /** The largest card width, tried first. */
  maxWidth: number
  /** The floor: below it the zone scrolls instead. */
  minWidth: number
  /** What the width drops by between two tries. */
  step: number
  /** Space between two boxes on a line. */
  gap: number
  /** Space between two lines, whether one row wrapped or the next row begins. */
  rowGap: number
}

/** The limits of the table: 112 px down to 44 in steps of 2. */
export const FIT_LIMITS: FitLimits = { maxWidth: 112, minWidth: 44, step: 2, gap: 6, rowGap: 4 }

/** One row at the chosen size. */
export interface RowFit {
  /** The stacks placed. */
  stacks: number
  /** The width of a box in px. */
  width: number
  /** The height of a box in px. */
  height: number
  /** The boxes on a full line; at least 1, even in a box narrower than one. */
  perLine: number
  /** The lines the row wraps into; 0 for an empty row. */
  lines: number
}

/** The answer of `fitRows`. */
export interface Fit {
  /** The zone's card width in px: the largest at which everything fits, else the floor. */
  cardWidth: number
  /** Each row at that width, in the order given. */
  layout: readonly RowFit[]
  /** The rows that hold at least one stack. */
  rows: number
  /** The lines of all rows together. */
  lines: number
  /** The height of all lines and the gaps between them in px; above the box exactly when `scrolls`. */
  height: number
  /** True when even the floor needs more height than the box has: the zone scrolls inside. */
  scrolls: boolean
  /** The stacks that end below the box with the zone scrolled to the top (the "+N" count); 0 unless `scrolls`. */
  hidden: number
}

// Every length is a whole CSS px, so the engine and the layout agree to the pixel. The box is floored (a
// ResizeObserver reports fractions, and a fraction of room is no room). A row's box is the zone's card width times
// its scale, rounded to the nearest px with halves up, and its height that width times its aspect, rounded the same
// way; callers draw exactly the sizes in `layout`. The limits are rounded too, and a step under 1 becomes 1.
const whole = (value: number): number => (Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0)
const rounded = (value: number, fallback: number): number =>
  Number.isFinite(value) ? Math.round(value) : fallback
const positive = (value: number | undefined, fallback: number): number =>
  value !== undefined && Number.isFinite(value) && value > 0 ? value : fallback

function placeRow(group: FitGroup, cardWidth: number, boxWidth: number, gap: number): RowFit {
  const stacks = whole(group.stacks)
  const width = Math.max(1, Math.round(cardWidth * positive(group.scale, 1)))
  const height = Math.max(1, Math.round(width * positive(group.aspect, CARD_ASPECT)))
  // A box narrower than one card still takes a card a line, as a wrapping row does.
  const perLine = Math.max(1, Math.floor((boxWidth + gap) / (width + gap)))
  return { stacks, width, height, perLine, lines: Math.ceil(stacks / perLine) }
}

function heightOf(layout: readonly RowFit[], rowGap: number): number {
  let lines = 0
  let height = 0
  for (const row of layout) {
    lines += row.lines
    height += row.lines * row.height
  }
  return lines === 0 ? 0 : height + (lines - 1) * rowGap
}

function hiddenBelow(layout: readonly RowFit[], boxHeight: number, rowGap: number): number {
  let hidden = 0
  let top = 0
  for (const row of layout) {
    for (let line = 0; line < row.lines; line += 1) {
      const bottom = top + row.height
      if (bottom > boxHeight) hidden += line < row.lines - 1 ? row.perLine : row.stacks - row.perLine * line
      top = bottom + rowGap
    }
  }
  return hidden
}

/**
 * The largest card width from `maxWidth` down to the floor, in steps, at which every row wraps inside the box width
 * and all lines fit its height. A fitting width stays fitting for a smaller one, so a bigger box never gives a smaller
 * card. At the floor without a fit the zone scrolls and `hidden` counts the stacks below the box.
 */
export function fitRows(box: FitBox, groups: readonly FitGroup[], limits: FitLimits = FIT_LIMITS): Fit {
  const minWidth = Math.max(1, rounded(limits.minWidth, FIT_LIMITS.minWidth))
  const maxWidth = Math.max(minWidth, rounded(limits.maxWidth, FIT_LIMITS.maxWidth))
  const step = Math.max(1, rounded(limits.step, FIT_LIMITS.step))
  const gap = Math.max(0, rounded(limits.gap, FIT_LIMITS.gap))
  const rowGap = Math.max(0, rounded(limits.rowGap, FIT_LIMITS.rowGap))
  const boxWidth = whole(box.width)
  const boxHeight = whole(box.height)

  const layoutAt = (cardWidth: number): RowFit[] =>
    groups.map((group) => placeRow(group, cardWidth, boxWidth, gap))
  let cardWidth = maxWidth
  let layout = layoutAt(cardWidth)
  while (cardWidth > minWidth && heightOf(layout, rowGap) > boxHeight) {
    cardWidth = Math.max(minWidth, cardWidth - step)
    layout = layoutAt(cardWidth)
  }
  const height = heightOf(layout, rowGap)
  const scrolls = height > boxHeight
  return {
    cardWidth,
    layout,
    rows: layout.filter((row) => row.lines > 0).length,
    lines: layout.reduce((sum, row) => sum + row.lines, 0),
    height,
    scrolls,
    hidden: scrolls ? hiddenBelow(layout, boxHeight, rowGap) : 0,
  }
}

/** Pure lane fit of a battlefield: the lanes a seat's permanents sit in and the largest card that fits them all. */
import type { RowKind } from './cardModel'
import { CARD_ASPECT, LAND_SCALE } from './fit'
import type { FitBox } from './fit'

/** How a board's lanes are stacked: one band of three lanes, creatures over lands and the rest, or a band each. */
export type BoardShape = 'one' | 'two' | 'three'

/**
 * The bands of each shape, from the middle of the table outwards, and the lanes of a band from left to right. Lands
 * are always at the left and other permanents at the right; creatures are nearest the middle whenever they have a band.
 */
export const SHAPES: Record<BoardShape, readonly (readonly RowKind[])[]> = {
  one: [['lands', 'creatures', 'other']],
  two: [['creatures'], ['lands', 'other']],
  three: [['creatures'], ['other'], ['lands']],
}

/** The shapes in the order a tie goes to: two bands read best, one band is next, a band each is the last resort. */
const PREFERENCE: readonly BoardShape[] = ['two', 'one', 'three']

/** The ladder of card widths and the spaces of a board, in CSS px. */
export interface LaneLimits {
  /** The largest card width; the top of the ladder. */
  maxWidth: number
  /** The floor: below it the zone scrolls instead. */
  minWidth: number
  /** The rungs of the ladder. */
  step: number
  /** Space between two boxes on a line. */
  gap: number
  /** Space between two lines of a lane. */
  rowGap: number
  /** Space between two lanes of a band. */
  laneGap: number
  /** Space between two bands. */
  bandGap: number
  /** The share of a land's height that shows above the next land of a fanned pile, at least `fanHeadMin` px. */
  fanHead: number
  /** The least px of a covered land that shows: its name line. */
  fanHeadMin: number
}

/** The limits of a board: 224 px down to 44 in steps of 2. */
export const LANE_LIMITS: LaneLimits = {
  maxWidth: 224,
  minWidth: 44,
  step: 2,
  gap: 6,
  rowGap: 4,
  laneGap: 12,
  bandGap: 12,
  fanHead: 0.14,
  fanHeadMin: 20,
}

/** The boxes each lane holds: one per stack, one per member of an open stack. */
export type LaneCounts = Record<RowKind, number>

/** One lane at the chosen size. */
export interface LanePlace {
  lane: RowKind
  /** The boxes placed. */
  boxes: number
  /** The box of a card in px. */
  width: number
  height: number
  /** The boxes on a full line and the lines of the lane; a fanned lane is one line of `columns` piles. */
  columns: number
  lines: number
  /** The width the lane takes in px: its columns and the gaps between them. */
  span: number
  /** The most lands in a fanned pile, each lying over the one before it below its head; 1 for a plain lane. */
  pile: number
  /** The px of a covered land of a pile that shows above the next: its name line; 0 for a plain lane. */
  head: number
  /** The height the lane takes in px. */
  tall: number
}

/** One band: lanes side by side. */
export interface BandPlace {
  lanes: readonly LanePlace[]
  /** The height of its tallest lane in px. */
  height: number
}

/** The answer of `fitLanes`. */
export interface LaneFit {
  shape: BoardShape
  /** The card width in px: the creatures'; lands and other permanents are `LAND_SCALE` of it. */
  cardWidth: number
  /** The bands from the top of the zone down. */
  bands: readonly BandPlace[]
  /** The height of all bands and the gaps between them in px. */
  height: number
  /** True when even the floor needs more height than the box has: the zone scrolls inside. */
  scrolls: boolean
  /** The boxes on lines that end below the box with the zone scrolled to the top; 0 unless `scrolls`. */
  hidden: number
}

const whole = (value: number): number => (Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0)

/** Creatures take the card width, lands and other permanents `LAND_SCALE` of it: creatures are the largest cards. */
export const scaleOf = (lane: RowKind): number => (lane === 'creatures' ? 1 : LAND_SCALE)

function placeLane(
  lane: RowKind,
  boxes: number,
  cardWidth: number,
  lines: number,
  limits: LaneLimits,
): LanePlace {
  const width = Math.max(1, Math.round(cardWidth * scaleOf(lane)))
  const height = Math.max(1, Math.round(width * CARD_ASPECT))
  const columns = Math.ceil(boxes / lines)
  const used = Math.ceil(boxes / columns)
  const span = columns * (width + limits.gap) - limits.gap
  const tall = used * height + (used - 1) * limits.rowGap
  return { lane, boxes, width, height, columns, lines: used, span, pile: 1, head: 0, tall }
}

/** The lands in `piles` fanned piles side by side, each land showing its head above the next. */
function fanLane(boxes: number, cardWidth: number, piles: number, limits: LaneLimits): LanePlace {
  const plain = placeLane('lands', boxes, cardWidth, 1, limits)
  const pile = Math.ceil(boxes / piles)
  const columns = Math.ceil(boxes / pile)
  const head = Math.min(plain.height, Math.max(limits.fanHeadMin, Math.round(plain.height * limits.fanHead)))
  const span = columns * (plain.width + limits.gap) - limits.gap
  return { ...plain, columns, lines: 1, span, pile, head, tall: plain.height + (pile - 1) * head }
}

const spanOf = (lanes: readonly LanePlace[], laneGap: number): number =>
  lanes.reduce((sum, lane) => sum + lane.span, 0) + laneGap * (lanes.length - 1)

const bandOf = (lanes: readonly LanePlace[]): BandPlace => ({
  lanes,
  height: Math.max(...lanes.map((lane) => lane.tall)),
})

/**
 * The lowest band whose lanes fit side by side in `boxWidth`: every lane in the fewest equal lines, or the lands
 * fanned into the most piles that fit beside the other lanes in one line, whichever is lower (plain on a tie); null
 * when neither fits.
 */
function placeBand(
  lanes: readonly RowKind[],
  counts: LaneCounts,
  cardWidth: number,
  boxWidth: number,
  limits: LaneLimits,
): BandPlace | null {
  const full = lanes.filter((lane) => counts[lane] > 0)
  const most = Math.max(...full.map((lane) => counts[lane]))
  let best: BandPlace | null = null
  for (let lines = 1; lines <= most && best === null; lines += 1) {
    const placed = full.map((lane) => placeLane(lane, counts[lane], cardWidth, lines, limits))
    if (spanOf(placed, limits.laneGap) <= boxWidth) best = bandOf(placed)
  }
  if (!full.includes('lands') || counts.lands < 2) return best
  for (let piles = counts.lands - 1; piles >= 1; piles -= 1) {
    const placed = full.map((lane) =>
      lane === 'lands'
        ? fanLane(counts.lands, cardWidth, piles, limits)
        : placeLane(lane, counts[lane], cardWidth, 1, limits),
    )
    if (spanOf(placed, limits.laneGap) > boxWidth) continue
    const fanned = bandOf(placed)
    return best === null || fanned.height < best.height ? fanned : best
  }
  return best
}

function placeShape(
  shape: BoardShape,
  counts: LaneCounts,
  cardWidth: number,
  boxWidth: number,
  limits: LaneLimits,
  facing: 'near' | 'far',
): BandPlace[] | null {
  const outwards = SHAPES[shape].filter((lanes) => lanes.some((lane) => counts[lane] > 0))
  // The viewer's board starts at the middle of the table, at its top; an opponent's ends there, at its bottom.
  const bands = facing === 'near' ? outwards : [...outwards].reverse()
  const placed: BandPlace[] = []
  for (const lanes of bands) {
    const band = placeBand(lanes, counts, cardWidth, whole(boxWidth), limits)
    if (band === null) return null
    placed.push(band)
  }
  return placed
}

const heightOf = (bands: readonly BandPlace[], bandGap: number): number =>
  bands.length === 0 ? 0 : bands.reduce((sum, band) => sum + band.height, 0) + bandGap * (bands.length - 1)

/**
 * The least height a board `boxWidth` wide needs to hold its lanes at `cardWidth`, over every shape; 0 for an empty
 * board and `Infinity` when no shape fits the width.
 */
export function laneHeight(
  boxWidth: number,
  counts: LaneCounts,
  cardWidth: number,
  limits: LaneLimits = LANE_LIMITS,
): number {
  let least = Number.POSITIVE_INFINITY
  for (const shape of PREFERENCE) {
    const bands = placeShape(shape, counts, cardWidth, boxWidth, limits, 'near')
    if (bands !== null) least = Math.min(least, heightOf(bands, limits.bandGap))
  }
  return least
}

function hiddenBelow(bands: readonly BandPlace[], boxHeight: number, limits: LaneLimits): number {
  let hidden = 0
  let top = 0
  for (const band of bands) {
    for (const lane of band.lanes) {
      if (lane.pile > 1) {
        if (top + lane.tall > boxHeight) hidden += lane.boxes
        continue
      }
      for (let line = 0; line < lane.lines; line += 1) {
        const bottom = top + (line + 1) * lane.height + line * limits.rowGap
        if (bottom > boxHeight) hidden += Math.min(lane.columns, lane.boxes - line * lane.columns)
      }
    }
    top += band.height + limits.bandGap
  }
  return hidden
}

/** The rungs of the ladder: the width of each and how many there are above the floor. */
export function ladderOf(limits: LaneLimits): { at: (rung: number) => number; rungs: number } {
  const step = Math.max(1, Math.round(limits.step))
  const minWidth = Math.max(1, Math.round(limits.minWidth))
  const rungs = Math.max(0, Math.floor((Math.round(limits.maxWidth) - minWidth) / step))
  return { at: (rung) => minWidth + rung * step, rungs }
}

/** The highest rung in `0..rungs` at which `fits` holds, by halving; `fits` holds at every rung under one where it does. */
export function highestRung(rungs: number, fits: (rung: number) => boolean): number {
  let low = 0
  let high = rungs
  while (low < high) {
    const middle = Math.ceil((low + high) / 2)
    if (fits(middle)) low = middle
    else high = middle - 1
  }
  return low
}

/**
 * The largest rung of the ladder at which some shape fits every lane inside the box, searched by halving: a shape
 * that fits at a width fits at every smaller one. Ties go to two bands, then one. At the floor without a fit the shape
 * that needs the least height wins and the zone scrolls.
 */
export function fitLanes(
  box: FitBox,
  counts: LaneCounts,
  facing: 'near' | 'far',
  limits: LaneLimits = LANE_LIMITS,
): LaneFit {
  const boxHeight = whole(box.height)
  const { at, rungs } = ladderOf(limits)
  const minWidth = at(0)
  const fits = (shape: BoardShape, rung: number): BandPlace[] | null => {
    const bands = placeShape(shape, counts, at(rung), box.width, limits, facing)
    return bands !== null && heightOf(bands, limits.bandGap) <= boxHeight ? bands : null
  }
  let best: LaneFit | null = null
  for (const shape of PREFERENCE) {
    if (fits(shape, 0) === null) continue
    const low = highestRung(rungs, (rung) => fits(shape, rung) !== null)
    if (best !== null && at(low) <= best.cardWidth) continue
    const bands = fits(shape, low) ?? []
    best = {
      shape,
      cardWidth: at(low),
      bands,
      height: heightOf(bands, limits.bandGap),
      scrolls: false,
      hidden: 0,
    }
  }
  if (best !== null) return best
  // Nothing fits at the floor: the shape that needs the least height, wrapping inside the width where it can. The
  // zone scrolls from its top, so creatures lead on every board, an opponent's too, and what is cut off is the rest.
  let floor: LaneFit | null = null
  for (const shape of PREFERENCE) {
    const bands = placeShape(shape, counts, minWidth, box.width, limits, 'near')
    if (bands === null) continue
    const height = heightOf(bands, limits.bandGap)
    if (floor !== null && height >= floor.height) continue
    floor = {
      shape,
      cardWidth: minWidth,
      bands,
      height,
      scrolls: true,
      hidden: hiddenBelow(bands, boxHeight, limits),
    }
  }
  if (floor !== null) return floor
  // A box narrower than one card a lane: a band each, one box a line.
  const bands = placeShape('three', counts, minWidth, Number.MAX_SAFE_INTEGER, limits, 'near') ?? []
  const narrow = bands.map((band) =>
    bandOf(band.lanes.map((lane) => placeLane(lane.lane, lane.boxes, minWidth, lane.boxes, limits))),
  )
  const height = heightOf(narrow, limits.bandGap)
  return {
    shape: 'three',
    cardWidth: minWidth,
    bands: narrow,
    height,
    scrolls: height > boxHeight,
    hidden: hiddenBelow(narrow, boxHeight, limits),
  }
}

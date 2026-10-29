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
}

/** The limits of a board: 224 px down to 44 in steps of 2. */
export const LANE_LIMITS: LaneLimits = {
  maxWidth: 224,
  minWidth: 44,
  step: 2,
  gap: 6,
  rowGap: 4,
  laneGap: 16,
  bandGap: 12,
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
  /** The boxes on a full line and the lines of the lane. */
  columns: number
  lines: number
  /** The width the lane takes in px: its columns and the gaps between them. */
  span: number
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
  /** The card width in px; lands are `LAND_SCALE` of it. */
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

const scaleOf = (lane: RowKind): number => (lane === 'lands' ? LAND_SCALE : 1)

function placeLane(lane: RowKind, boxes: number, cardWidth: number, lines: number, gap: number): LanePlace {
  const width = Math.max(1, Math.round(cardWidth * scaleOf(lane)))
  const height = Math.max(1, Math.round(width * CARD_ASPECT))
  const columns = Math.ceil(boxes / lines)
  const used = Math.ceil(boxes / columns)
  return { lane, boxes, width, height, columns, lines: used, span: columns * (width + gap) - gap }
}

/** The band in the fewest lines whose lanes fit side by side in `boxWidth`; null when one box a lane is too wide. */
function placeBand(
  lanes: readonly RowKind[],
  counts: LaneCounts,
  cardWidth: number,
  boxWidth: number,
  limits: LaneLimits,
): BandPlace | null {
  const full = lanes.filter((lane) => counts[lane] > 0)
  const most = Math.max(...full.map((lane) => counts[lane]))
  for (let lines = 1; lines <= most; lines += 1) {
    const placed = full.map((lane) => placeLane(lane, counts[lane], cardWidth, lines, limits.gap))
    const span = placed.reduce((sum, lane) => sum + lane.span, 0) + limits.laneGap * (placed.length - 1)
    if (span <= boxWidth) {
      const height = Math.max(
        ...placed.map((lane) => lane.lines * lane.height + (lane.lines - 1) * limits.rowGap),
      )
      return { lanes: placed, height }
    }
  }
  return null
}

function placeShape(
  shape: BoardShape,
  counts: LaneCounts,
  cardWidth: number,
  box: FitBox,
  limits: LaneLimits,
  facing: 'near' | 'far',
): BandPlace[] | null {
  const outwards = SHAPES[shape].filter((lanes) => lanes.some((lane) => counts[lane] > 0))
  // The viewer's board starts at the middle of the table, at its top; an opponent's ends there, at its bottom.
  const bands = facing === 'near' ? outwards : [...outwards].reverse()
  const placed: BandPlace[] = []
  for (const lanes of bands) {
    const band = placeBand(lanes, counts, cardWidth, whole(box.width), limits)
    if (band === null) return null
    placed.push(band)
  }
  return placed
}

const heightOf = (bands: readonly BandPlace[], bandGap: number): number =>
  bands.length === 0 ? 0 : bands.reduce((sum, band) => sum + band.height, 0) + bandGap * (bands.length - 1)

function hiddenBelow(bands: readonly BandPlace[], boxHeight: number, limits: LaneLimits): number {
  let hidden = 0
  let top = 0
  for (const band of bands) {
    for (const lane of band.lanes) {
      for (let line = 0; line < lane.lines; line += 1) {
        const bottom = top + (line + 1) * lane.height + line * limits.rowGap
        if (bottom > boxHeight) hidden += Math.min(lane.columns, lane.boxes - line * lane.columns)
      }
    }
    top += band.height + limits.bandGap
  }
  return hidden
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
  const step = Math.max(1, Math.round(limits.step))
  const minWidth = Math.max(1, Math.round(limits.minWidth))
  const rungs = Math.max(0, Math.floor((Math.round(limits.maxWidth) - minWidth) / step))
  const at = (rung: number): number => minWidth + rung * step
  const fits = (shape: BoardShape, rung: number): BandPlace[] | null => {
    const bands = placeShape(shape, counts, at(rung), box, limits, facing)
    return bands !== null && heightOf(bands, limits.bandGap) <= boxHeight ? bands : null
  }
  let best: LaneFit | null = null
  for (const shape of PREFERENCE) {
    if (fits(shape, 0) === null) continue
    let low = 0
    let high = rungs
    while (low < high) {
      const middle = Math.ceil((low + high) / 2)
      if (fits(shape, middle) === null) high = middle - 1
      else low = middle
    }
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
    const bands = placeShape(shape, counts, minWidth, box, limits, 'near')
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
  const bands =
    placeShape('three', counts, minWidth, { width: Number.MAX_SAFE_INTEGER, height: 0 }, limits, 'near') ?? []
  const narrow = bands.map((band) => {
    const lanes = band.lanes.map((lane) => placeLane(lane.lane, lane.boxes, minWidth, lane.boxes, limits.gap))
    return {
      lanes,
      height: Math.max(...lanes.map((lane) => lane.lines * lane.height + (lane.lines - 1) * limits.rowGap)),
    }
  })
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

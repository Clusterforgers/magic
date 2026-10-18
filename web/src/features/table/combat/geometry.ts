/** Line geometry for the combat overlay. All of it is pure: boxes in, SVG path text out. */

/** A point in the overlay's own coordinates (pixels from its top left corner). */
export interface Point {
  x: number
  y: number
}

/** A box in the overlay's coordinates. */
export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

/** A curve joining two boxes and its arrowhead. */
export interface Shape {
  /** The curve as SVG path data; it stops inside the arrowhead so the stroke never pokes out of its tip. */
  d: string
  /** The arrowhead as SVG path data of a closed triangle. */
  head: string
  /** Where the line leaves the first box. */
  start: Point
  /** The tip of the arrowhead, where the line reaches the second box. */
  end: Point
  /** The middle of the curve. */
  mid: Point
}

/** How a line is drawn between two boxes. */
export interface ConnectOptions {
  /** Pixels kept between a line and the boxes it joins; 3 by default. */
  gap?: number
  /** How far the curve bows, as a share of the distance (the sign picks the side); 0.12 by default. */
  bow?: number
  /** Length of the arrowhead in pixels; 9 by default. */
  head?: number
}

const DEFAULTS: Required<ConnectOptions> = { gap: 3, bow: 0.12, head: 9 }
// Half the opening of the arrowhead (28 degrees), and how deep inside the head the curve ends.
const SPREAD = (28 * Math.PI) / 180
const TRIM = 0.7

/** A coordinate with one decimal and no "-0", so equal drawings give equal text. */
function num(value: number): string {
  const rounded = Math.round(value * 10) / 10
  return Object.is(rounded, -0) ? '0' : String(rounded)
}

const at = (point: Point): string => `${num(point.x)} ${num(point.y)}`

/** True for a box that has been laid out: finite numbers with some room inside. */
export function isLaidOut(rect: Rect): boolean {
  return [rect.x, rect.y, rect.width, rect.height].every(Number.isFinite) && rect.width > 0 && rect.height > 0
}

/** The middle of a box. */
export function centreOf(rect: Rect): Point {
  return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }
}

/**
 * Where a straight line from the middle of `rect` toward `toward` leaves the box, `gap` pixels further out. Null when
 * the line goes nowhere (same point) or `toward` lies inside the box, where there is nothing to leave.
 */
export function exitPoint(rect: Rect, toward: Point, gap: number): Point | null {
  const from = centreOf(rect)
  const dx = toward.x - from.x
  const dy = toward.y - from.y
  const distance = Math.hypot(dx, dy)
  if (distance === 0) return null
  const scaleX = dx === 0 ? Infinity : rect.width / 2 / Math.abs(dx)
  const scaleY = dy === 0 ? Infinity : rect.height / 2 / Math.abs(dy)
  const scale = Math.min(scaleX, scaleY)
  if (scale >= 1) return null
  const reach = scale + gap / distance
  return { x: from.x + dx * reach, y: from.y + dy * reach }
}

/**
 * A curve from the edge of one box to the edge of another, with an arrowhead at the second. Null when either box has
 * not been laid out, or when the boxes overlap so that there is no space between them to draw in.
 */
export function connect(from: Rect, to: Rect, options: ConnectOptions = {}): Shape | null {
  if (!isLaidOut(from) || !isLaidOut(to)) return null
  const { gap, bow, head } = { ...DEFAULTS, ...options }
  const a = centreOf(from)
  const b = centreOf(to)
  const start = exitPoint(from, b, gap)
  const end = exitPoint(to, a, gap)
  if (start === null || end === null) return null
  const dx = end.x - start.x
  const dy = end.y - start.y
  const length = Math.hypot(dx, dy)
  // Boxes that overlap leave the two ends crossed over: the line would run backwards.
  if (length < 1 || dx * (b.x - a.x) + dy * (b.y - a.y) <= 0) return null

  const mid = { x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 }
  const control = { x: mid.x - dy * bow, y: mid.y + dx * bow }
  // Cut the curve short, inside the arrowhead, by splitting it at `t` (de Casteljau).
  const t = Math.max(0.5, 1 - (TRIM * head) / length)
  const cut: Point = {
    x: (1 - t) * (1 - t) * start.x + 2 * (1 - t) * t * control.x + t * t * end.x,
    y: (1 - t) * (1 - t) * start.y + 2 * (1 - t) * t * control.y + t * t * end.y,
  }
  const handle = { x: start.x + t * (control.x - start.x), y: start.y + t * (control.y - start.y) }
  const angle = Math.atan2(end.y - control.y, end.x - control.x)
  const wing = (turn: number): Point => ({
    x: end.x - head * Math.cos(angle + turn),
    y: end.y - head * Math.sin(angle + turn),
  })
  return {
    d: `M ${at(start)} Q ${at(handle)} ${at(cut)}`,
    head: `M ${at(end)} L ${at(wing(-SPREAD))} L ${at(wing(SPREAD))} Z`,
    start,
    end,
    mid: {
      x: 0.25 * start.x + 0.5 * control.x + 0.25 * end.x,
      y: 0.25 * start.y + 0.5 * control.y + 0.25 * end.y,
    },
  }
}

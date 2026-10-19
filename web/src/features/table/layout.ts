/** Pure layout of the table: the sizes of its regions for a window, the mode for a seat count, and who is drawn where. */
import type { PlayerId } from '@/protocol/generated'

/** Grid mode draws every opponent's battlefield; focus mode draws one large and the others as tiles. */
export type TableMode = 'grid' | 'focus'

/** How the opponents are arranged: one wide board beside a pod column, columns of one-line pods, or a tile strip and one board. */
export type SeatShape = 'duel' | 'columns' | 'focus'

/** Up to this many seats a table starts in grid mode. */
export const GRID_MAX_SEATS = 4
/** From this many seats cards are drawn in the compact variant. */
export const COMPACT_FROM_SEATS = 8
/** The most columns grid mode uses; more opponents wrap onto further rows. */
export const GRID_MAX_COLUMNS = 3

/** What the default focus is chosen from. */
export interface FocusContext {
  /** The seat whose turn it is. */
  active: PlayerId | null
  /** The seat that holds priority. */
  holder: PlayerId | null
  /** Seats that have lost; the fallback skips them. */
  eliminated: readonly PlayerId[]
}

/** The seats of a table and how they are drawn. */
export interface SeatPlan {
  mode: TableMode
  /** The arrangement of the opponents: `duel` for one opponent in grid mode, `columns` for more, `focus` for the strip. */
  shape: SeatShape
  /** The seat this client plays; drawn last, at the bottom. */
  viewer: PlayerId
  /** Every other seat in turn order, starting after the viewer. */
  opponents: PlayerId[]
  /** The opponent drawn large, in focus mode only. */
  focused: PlayerId | null
  /** Opponents whose battlefield is drawn: all of them in grid mode, only the focused one in focus mode. */
  boards: PlayerId[]
  /** Opponents drawn as compact tiles: all of them in focus mode, none in grid mode. */
  tiles: PlayerId[]
  /** Columns for the opponents' boards. */
  columns: 1 | 2 | 3
  /** Whether cards are drawn in the compact variant. */
  compact: boolean
}

/** What `arrangeSeats` needs. */
export interface SeatInput {
  seatCount: number
  viewer: PlayerId
  /** The mode to draw; the default for the seat count when left out. */
  mode?: TableMode
  /** The opponent the player chose to focus; ignored when it is the viewer or not at the table. */
  focus?: PlayerId | null
  context?: FocusContext
}

const NO_CONTEXT: FocusContext = { active: null, holder: null, eliminated: [] }

/** Grid mode for up to `GRID_MAX_SEATS` seats, focus mode for more. */
export function defaultMode(seatCount: number): TableMode {
  return seatCount <= GRID_MAX_SEATS ? 'grid' : 'focus'
}

/** Every seat but `viewer`, in turn order from the seat after it. */
export function opponentsOf(seatCount: number, viewer: PlayerId): PlayerId[] {
  if (seatCount < 2) return []
  return Array.from({ length: seatCount - 1 }, (_, index) => (viewer + 1 + index) % seatCount)
}

/** The opponent to show large when nobody chose one: the active player, else the priority holder, else the next seat. */
export function defaultFocus(
  seatCount: number,
  viewer: PlayerId,
  context: FocusContext = NO_CONTEXT,
): PlayerId | null {
  const opponents = opponentsOf(seatCount, viewer)
  const { active, holder, eliminated } = context
  if (active !== null && opponents.includes(active)) return active
  if (holder !== null && opponents.includes(holder)) return holder
  return opponents.find((seat) => !eliminated.includes(seat)) ?? opponents[0] ?? null
}

const wholeSeats = (seatCount: number): number =>
  Number.isFinite(seatCount) ? Math.max(0, Math.floor(seatCount)) : 0

/** Arranges the table; total for any input, so a bad seat number never throws. */
export function arrangeSeats(input: SeatInput): SeatPlan {
  const seatCount = wholeSeats(input.seatCount)
  const viewer = seatCount === 0 ? 0 : ((Math.trunc(input.viewer) % seatCount) + seatCount) % seatCount
  const mode = input.mode ?? defaultMode(seatCount)
  const opponents = opponentsOf(seatCount, viewer)
  const compact = seatCount >= COMPACT_FROM_SEATS
  if (mode === 'grid') {
    const columns = Math.min(GRID_MAX_COLUMNS, Math.max(1, opponents.length))
    return {
      mode,
      shape: opponents.length <= 1 ? 'duel' : 'columns',
      viewer,
      opponents,
      focused: null,
      boards: opponents,
      tiles: [],
      columns: toColumns(columns),
      compact,
    }
  }
  const chosen = input.focus ?? null
  const focused =
    chosen !== null && opponents.includes(chosen) ? chosen : defaultFocus(seatCount, viewer, input.context)
  return {
    mode,
    shape: 'focus',
    viewer,
    opponents,
    focused,
    boards: focused === null ? [] : [focused],
    tiles: opponents,
    columns: 1,
    compact,
  }
}

function toColumns(columns: number): 1 | 2 | 3 {
  return columns >= 3 ? 3 : columns === 2 ? 2 : 1
}

// ---- Regions of the window ------------------------------------------------------------------------------------

/** The window, in CSS px. */
export interface Viewport {
  width: number
  height: number
}

/** The width classes of the design system: XS under 640, S 640 to 1023, M 1024 to 1279, L 1280 to 1599, XL 1600 up. */
export type Breakpoint = 'xs' | 's' | 'm' | 'l' | 'xl'

/** The smallest window width of each breakpoint. */
export const BREAKPOINT_FROM: Record<Breakpoint, number> = { xs: 0, s: 640, m: 1024, l: 1280, xl: 1600 }

/** The breakpoint of a window width. */
export function breakpointOf(width: number): Breakpoint {
  if (width >= BREAKPOINT_FROM.xl) return 'xl'
  if (width >= BREAKPOINT_FROM.l) return 'l'
  if (width >= BREAKPOINT_FROM.m) return 'm'
  if (width >= BREAKPOINT_FROM.s) return 's'
  return 'xs'
}

/** How the side area (log, chat and stack) is drawn: a column, a rail that opens over the table, or a sheet. */
export type SideKind = 'column' | 'rail' | 'sheet'

const SIDE_OF: Record<Breakpoint, SideKind> = {
  xs: 'sheet',
  s: 'sheet',
  m: 'rail',
  l: 'column',
  xl: 'column',
}

/** A length that follows the window's height: `vh` percent of it between `min` and `max` px (CSS `clamp`). */
export interface HeightClamp {
  min: number
  vh: number
  max: number
}

/** The top bar in px, and on a phone held on its side. */
export const TOP_BAR_PX = 44
export const TOP_BAR_LANDSCAPE_PX = 32
/** The shared strip between the opponents and the own board. */
export const SHARED_CLAMP: HeightClamp = { min: 56, vh: 9, max: 88 }
/** The hand dock; a phone held upright gets the lower one. */
export const HAND_CLAMP: HeightClamp = { min: 112, vh: 17, max: 200 }
export const HAND_PHONE_CLAMP: HeightClamp = { min: 96, vh: 14, max: 120 }
/** The own board is this many times as tall as the opponents' area (`minmax(0, 1.15fr)` against `minmax(0, 1fr)`). */
export const OWN_SHARE = 1.15
/** The tallest a long prompt grows, in percent of the window height. */
export const SHEET_MAX_VH = 40
/** The side column takes this share of the window width between its bounds; M gets a rail of `RAIL_PX`. */
export const SIDE_SHARE = 0.22
export const SIDE_MIN_PX = 280
export const SIDE_MAX_PX = 320
export const RAIL_PX = 44
/** A window lower than this and wider than it is high is a phone on its side. */
export const LANDSCAPE_MAX_HEIGHT = 500
/**
 * The narrowest window whose controls dock draws the stop ladder and Full Control as one row above the pass buttons
 * (L and up). A narrower dock holds the same content behind its "Stops" button.
 */
export const STOPS_ROW_MIN_WIDTH_PX = BREAKPOINT_FROM.l

/** The sizes of the table's regions in a window, in px. Rounded nowhere, as CSS is not. */
export interface Regions {
  breakpoint: Breakpoint
  /** A narrow window (XS): the pod strip, the focused board and a lower hand dock. */
  phone: boolean
  /** A phone on its side: the S layout with a lower top bar. */
  landscape: boolean
  side: SideKind
  /** What the side column or rail takes of the width; 0 for a sheet. */
  sideWidth: number
  /** The area left of the side column. */
  main: Viewport
  topBar: number
  shared: number
  hand: number
  opponents: number
  own: number
  /**
   * The most a prompt sheet grows to: 40 % of the height, up over the opponents' board and no further than the top
   * bar. Never down over your board (the cards a prompt asks you to click are yours), the hand dock or the controls.
   */
  sheet: number
}

const finite = (value: number): number => (Number.isFinite(value) ? Math.max(0, value) : 0)
const clampTo = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value))

/** The px a `HeightClamp` gives in a window of this height. */
export function clampedHeight(range: HeightClamp, height: number): number {
  return clampTo((height * range.vh) / 100, range.min, range.max)
}

/** Sizes the regions of the table for a window; total for any input. */
export function regionSizes(viewport: Viewport): Regions {
  const width = finite(viewport.width)
  const height = finite(viewport.height)
  const breakpoint = breakpointOf(width)
  const phone = breakpoint === 'xs'
  const landscape = height < LANDSCAPE_MAX_HEIGHT && width > height
  const side = SIDE_OF[breakpoint]
  const sideWidth =
    side === 'column' ? clampTo(width * SIDE_SHARE, SIDE_MIN_PX, SIDE_MAX_PX) : side === 'rail' ? RAIL_PX : 0
  const topBar = landscape ? TOP_BAR_LANDSCAPE_PX : TOP_BAR_PX
  const shared = clampedHeight(SHARED_CLAMP, height)
  const hand = clampedHeight(phone ? HAND_PHONE_CLAMP : HAND_CLAMP, height)
  const rest = Math.max(0, height - topBar - shared - hand)
  const opponents = rest / (1 + OWN_SHARE)
  const own = rest - opponents
  return {
    breakpoint,
    phone,
    landscape,
    side,
    sideWidth,
    main: { width: width - sideWidth, height },
    topBar,
    shared,
    hand,
    opponents,
    own,
    sheet: Math.min((height * SHEET_MAX_VH) / 100, shared + opponents),
  }
}

const clampCss = (range: HeightClamp): string => `clamp(${range.min}px, ${range.vh}dvh, ${range.max}px)`

/** The `grid-template-rows` of the table's main area: top bar, opponents, shared strip, own board, hand dock. */
export function gridRows(regions: Pick<Regions, 'phone' | 'landscape'>): string {
  const bar = regions.landscape ? TOP_BAR_LANDSCAPE_PX : TOP_BAR_PX
  const hand = clampCss(regions.phone ? HAND_PHONE_CLAMP : HAND_CLAMP)
  return `${bar}px minmax(0, 1fr) ${clampCss(SHARED_CLAMP)} minmax(0, ${OWN_SHARE}fr) ${hand}`
}

/**
 * The widest the controls dock gets, in px: the pass buttons in one row; with the stop ladder and Full Control in a
 * row of their own above them; a phone's is a column for the pass button.
 */
export const CONTROLS_MAX_PX = 480
export const CONTROLS_STOPS_MAX_PX = 720
export const CONTROLS_MAX_PHONE_PX = 160

/** The `grid-template-columns` of the main area: the controls dock is as wide as its content up to its limit and the hand dock has the rest. */
export function mainColumns(regions: Pick<Regions, 'phone'>, stopsRow = false): string {
  const widest = regions.phone ? CONTROLS_MAX_PHONE_PX : stopsRow ? CONTROLS_STOPS_MAX_PX : CONTROLS_MAX_PX
  return `minmax(0, 1fr) fit-content(${widest}px)`
}

/** The `grid-template-columns` of the table: the main area, then the side column or rail. */
export function gridColumns(side: SideKind): string {
  if (side === 'column') {
    return `minmax(0, 1fr) clamp(${SIDE_MIN_PX}px, ${SIDE_SHARE * 100}vw, ${SIDE_MAX_PX}px)`
  }
  return side === 'rail' ? `minmax(0, 1fr) ${RAIL_PX}px` : 'minmax(0, 1fr)'
}

// ---- Mode and shape ----------------------------------------------------------------------------------------------

/** Space around the zones of the table and between them, in px. */
export const ZONE_GAP_PX = 8
/** The width of a seat's pod beside its board. */
export const POD_COLUMN_PX = 168
/** A board column narrower than this is not drawn: the table switches to focus mode. */
export const MIN_COLUMN_PX = 280

/** The width of the narrowest board grid mode would draw for `seatCount` seats in a main area `mainWidth` wide. */
export function gridColumnWidth(seatCount: number, mainWidth: number): number {
  const opponents = Math.max(1, wholeSeats(seatCount) - 1)
  const inner = finite(mainWidth) - 2 * ZONE_GAP_PX
  // One opponent: a wide board beside the pod column. More: equal columns of one-line pods.
  if (opponents === 1) return inner - POD_COLUMN_PX - ZONE_GAP_PX
  const columns = Math.min(GRID_MAX_COLUMNS, opponents)
  return (inner - (columns - 1) * ZONE_GAP_PX) / columns
}

/** What `chooseMode` needs. */
export interface ModeInput {
  seatCount: number
  /** The width of the main area, from `regionSizes`. */
  mainWidth: number
  /** The mode the player chose from the menu, which wins over the rule. */
  override?: TableMode | undefined
}

/** Grid for 2 to 4 seats, focus for 5 or more or when a grid column would be under `MIN_COLUMN_PX`; the player may override. */
export function chooseMode({ seatCount, mainWidth, override }: ModeInput): TableMode {
  if (override !== undefined) return override
  if (defaultMode(wholeSeats(seatCount)) === 'focus') return 'focus'
  return gridColumnWidth(seatCount, mainWidth) < MIN_COLUMN_PX ? 'focus' : 'grid'
}

/** Where a seat's pod goes: beside its board as a column, above it as one line, or nowhere (a tile already says it). */
export type PodPlace = 'aside' | 'line' | 'none'

/** What `planTable` needs. */
export interface TableInput extends Omit<SeatInput, 'mode'> {
  viewport: Viewport
  /** The mode the player chose, if any. */
  mode?: TableMode | undefined
  /** The seat whose turn it is; on a phone its tile leads the strip. */
  active?: PlayerId | null | undefined
}

/** `tiles` with `active` moved to the front, the rest as they were; unchanged when `active` is not among them. */
export function activeFirst(tiles: readonly PlayerId[], active: PlayerId | null | undefined): PlayerId[] {
  if (active === null || active === undefined || !tiles.includes(active)) return [...tiles]
  return [active, ...tiles.filter((seat) => seat !== active)]
}

/** The whole layout of the table for a window and a seat count. */
export interface TablePlan {
  regions: Regions
  seats: SeatPlan
  /** Your own pod: a column beside your board, or a line over it on a narrow window. */
  pod: PodPlace
  /** The pod of the opponent whose board is drawn large: as yours, but none on a phone, where the tile above the board has the figures. */
  opponentPod: PodPlace
  /** Whether the controls dock draws the stop ladder and Full Control as one row; otherwise they open from its "Stops" button. */
  stopsRow: boolean
}

/** Sizes the regions, chooses the mode and arranges the seats. */
export function planTable(input: TableInput): TablePlan {
  const regions = regionSizes(input.viewport)
  const seatCount = wholeSeats(input.seatCount)
  const mode = chooseMode({ seatCount, mainWidth: regions.main.width, override: input.mode })
  const arranged = arrangeSeats({ ...input, seatCount, mode })
  // The strip of a phone shows a few tiles at a time: the seat whose turn it is leads it.
  const seats = regions.phone ? { ...arranged, tiles: activeFirst(arranged.tiles, input.active) } : arranged
  const pod: PodPlace = regions.phone ? 'line' : 'aside'
  return {
    regions,
    seats,
    pod,
    opponentPod: regions.phone && seats.shape === 'focus' ? 'none' : pod,
    stopsRow: BREAKPOINT_FROM[regions.breakpoint] >= STOPS_ROW_MIN_WIDTH_PX,
  }
}

/** Pure layout of the table: the sizes of its regions for a window, the mode for a seat count, and who is drawn where. */
import type { PlayerId } from '@/protocol/generated'
import type { DockForm } from './dockLayout'
import { handRowPx } from './handFit'

/**
 * Grid mode draws every opponent's battlefield; focus mode draws one large and the others as tiles; boss mode (a
 * co-op fight against a boss) draws the boss's side on top and the party together at the bottom.
 */
export type TableMode = 'grid' | 'focus' | 'boss'

/**
 * How the opponents are arranged: one wide board beside a pod column, columns of one-line pods, a tile strip and one
 * board, or the boss's side (the boss wide, its minions beside it).
 */
export type SeatShape = 'duel' | 'columns' | 'focus' | 'boss'

/** The two sides of a co-op fight against a boss: the boss's seat and every seat of its side, the boss first. */
export interface BossSides {
  boss: PlayerId
  enemies: readonly PlayerId[]
}

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
  /** The viewer's teammates in boss mode, drawn beside the viewer at the bottom; none in the other modes. */
  allies: PlayerId[]
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
  /** The sides of a fight against a boss; boss mode needs them and is the default with them. */
  sides?: BossSides | null
}

const NO_CONTEXT: FocusContext = { active: null, holder: null, eliminated: [] }

/** Grid mode for up to `GRID_MAX_SEATS` seats, focus mode for more. */
export function defaultMode(seatCount: number): TableMode {
  return seatCount <= GRID_MAX_SEATS ? 'grid' : 'focus'
}

/** `sides` when the viewer plays against the boss (or is not known); null otherwise, or without a boss. */
export function bossSidesFor(sides: BossSides | null | undefined, viewer?: PlayerId): BossSides | null {
  if (sides === null || sides === undefined || sides.enemies.length === 0) return null
  return viewer !== undefined && sides.enemies.includes(viewer) ? null : sides
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
  const sides = bossSidesFor(input.sides, viewer)
  const asked = input.mode ?? (sides === null ? defaultMode(seatCount) : 'boss')
  // Boss mode needs a boss; without one the table falls back to the rule of the seat count.
  const mode = asked === 'boss' && sides === null ? defaultMode(seatCount) : asked
  const opponents = opponentsOf(seatCount, viewer)
  const compact = seatCount >= COMPACT_FROM_SEATS
  if (mode === 'boss' && sides !== null) {
    const enemies = [sides.boss, ...sides.enemies.filter((seat) => seat !== sides.boss)].filter((seat) =>
      opponents.includes(seat),
    )
    const chosen = input.focus ?? null
    return {
      mode,
      shape: 'boss',
      viewer,
      opponents,
      focused: chosen !== null && enemies.includes(chosen) ? chosen : (enemies[0] ?? null),
      boards: enemies,
      tiles: [],
      allies: opponents.filter((seat) => !enemies.includes(seat)),
      columns: toColumns(enemies.length),
      compact,
    }
  }
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
      allies: [],
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
    allies: [],
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
export const SHARED_CLAMP: HeightClamp = { min: 56, vh: 7, max: 72 }
/** The hand dock; a phone held upright gets the lower one. */
export const HAND_CLAMP: HeightClamp = { min: 96, vh: 13, max: 168 }
export const HAND_PHONE_CLAMP: HeightClamp = { min: 96, vh: 14, max: 120 }
/**
 * A hand that one row does not hold grows the dock (not a phone's) to the height of its rows, up to this percent of the
 * window height (`HAND_GROW_VAR`, `handDockNeed`); the boards give up that room.
 */
export const HAND_GROW_VH = 23
/**
 * While an opening hand waits for its keep or mulligan the dock may take this percent of the window height to draw the
 * hand large (`openingDockNeed`): the boards are empty then.
 */
export const OPENING_GROW_VH = 32
/** The custom property of the table's main area that holds the height the hand dock needs for its rows, in px. */
export const HAND_GROW_VAR = '--hand-grow'
/** The own board is this many times as tall as the opponents' area (`minmax(0, 1.15fr)` against `minmax(0, 1fr)`). */
export const OWN_SHARE = 1.15
/** The tallest a long prompt grows, in percent of the window height. */
export const SHEET_MAX_VH = 40
/** What a phone's prompt sheet leaves of the opponents' area: the tile strip and a row of their board, in px. */
export const SHEET_KEEP_PX = 120
/** The side column takes this share of the window width between its bounds; M gets a rail of `RAIL_PX`. */
export const SIDE_SHARE = 0.22
export const SIDE_MIN_PX = 280
export const SIDE_MAX_PX = 320
export const RAIL_PX = 44
/** A window lower than this and wider than it is high is a phone on its side. */
export const LANDSCAPE_MAX_HEIGHT = 500
/**
 * The narrowest window whose controls dock draws the stop ladder and Full Control as one row above the pass buttons
 * (XL and up): at 1280 the hand dock beside it would get a third of the width. A narrower dock holds the same content
 * behind its "Stops" button.
 */
export const STOPS_ROW_MIN_WIDTH_PX = BREAKPOINT_FROM.xl
/** The window width from which the step ladder, which also sets the stops, sits in the top bar; below it the stops popover holds it. */
export const LADDER_IN_BAR_MIN_WIDTH_PX = BREAKPOINT_FROM.m

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
   * The most a prompt sheet grows to: 40 % of the height, up over the opponents' board (on a phone leaving
   * `SHEET_KEEP_PX` of it, elsewhere their pods and `BOARD_KEEP_PX` of their boards once split) and no further than the top bar. Never down over your board (the cards a prompt asks you to click are yours), the hand dock or the controls.
   */
  sheet: number
}

const finite = (value: number): number => (Number.isFinite(value) ? Math.max(0, value) : 0)
const clampTo = (value: number, min: number, max: number): number => Math.min(max, Math.max(min, value))

/**
 * The most a prompt sheet grows to in a window of this height, over a strip and an opponents' area of these heights:
 * it leaves `SHEET_KEEP_PX` of the area on a phone and `keep` px elsewhere.
 */
export function sheetMost(
  height: number,
  shared: number,
  opponents: number,
  phone: boolean,
  keep = 0,
): number {
  const room = Math.max(0, opponents - (phone ? SHEET_KEEP_PX : keep))
  return Math.min((height * SHEET_MAX_VH) / 100, shared + room)
}

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
    sheet: sheetMost(height, shared, opponents, phone),
  }
}

const clampCss = (range: HeightClamp): string => `clamp(${range.min}px, ${range.vh}dvh, ${range.max}px)`

/**
 * The `grid-template-rows` of the table's main area: top bar, opponents, shared strip, own board, hand dock. The two
 * board rows share what is left by the weights of `opponents` and `own` (a split, `boardSplit.ts`), else 1 to `OWN_SHARE`.
 */
export function gridRows(
  regions: Pick<Regions, 'phone' | 'landscape'> & Partial<Pick<Regions, 'opponents' | 'own'>>,
): string {
  const bar = regions.landscape ? TOP_BAR_LANDSCAPE_PX : TOP_BAR_PX
  const hand = regions.phone
    ? clampCss(HAND_PHONE_CLAMP)
    : `max(${clampCss(HAND_CLAMP)}, var(${HAND_GROW_VAR}, 0px))`
  const weighted = (regions.opponents ?? 0) > 0 && (regions.own ?? 0) > 0
  const [theirs, own] = weighted ? [weight(regions.opponents), weight(regions.own)] : [1, OWN_SHARE]
  return `${bar}px minmax(0, ${theirs}fr) ${clampCss(SHARED_CLAMP)} minmax(0, ${own}fr) ${hand}`
}

/** The tallest the hand dock grows for its rows in a window of these regions, in px; 0 on a phone, whose dock keeps its size. */
export function handGrowMost(regions: Pick<Regions, 'phone' | 'main'>): number {
  return regions.phone ? 0 : (regions.main.height * HAND_GROW_VH) / 100
}

/** The tallest the hand dock grows for an opening hand in a window of these regions, in px; 0 on a phone, whose dock keeps its size. */
export function openingGrowMost(regions: Pick<Regions, 'phone' | 'main'>): number {
  return regions.phone ? 0 : (regions.main.height * OPENING_GROW_VH) / 100
}

const weight = (px: number | undefined): number => Math.round((px ?? 0) * 100) / 100

/**
 * The widest the controls dock gets, in px: the pass buttons in one row; with the stop ladder and Full Control in a
 * row of their own above them; a phone's is a column for the pass button.
 */
export const CONTROLS_MAX_PX = 480
export const CONTROLS_STOPS_MAX_PX = 760
export const CONTROLS_MAX_PHONE_PX = 160

/** The `grid-template-columns` of the main area: the controls dock is as wide as its content up to its limit and the hand dock has the rest. */
export function mainColumns(regions: Pick<Regions, 'phone'>, stopsRow = false): string {
  const widest = regions.phone ? CONTROLS_MAX_PHONE_PX : stopsRow ? CONTROLS_STOPS_MAX_PX : CONTROLS_MAX_PX
  return `minmax(0, 1fr) fit-content(${widest}px)`
}

/** The cards of the hand the controls dock leaves one row for: an opening hand, with a pair of partners to cast beside it. */
const DOCK_HAND_CARDS = 7
const DOCK_HAND_COMMANDERS = 2

/**
 * The narrowest main area in px whose hand dock holds an opening hand and two commanders in one row beside the widest
 * controls dock: the room for the key hints and the whole words of the primary button.
 */
export const DOCK_FULL_MIN_MAIN_PX = handRowPx(DOCK_HAND_CARDS, DOCK_HAND_COMMANDERS) + CONTROLS_MAX_PX

/** How much the controls dock writes in these regions: all of it where the hand keeps its row, less where it would not, the least on a phone. */
export function dockForm(regions: Pick<Regions, 'phone' | 'main'>): DockForm {
  if (regions.phone) return 'compact'
  return regions.main.width >= DOCK_FULL_MIN_MAIN_PX ? 'full' : 'short'
}

/** The `grid-template-columns` of the table: the main area, then the side column or rail. */
export function gridColumns(side: SideKind): string {
  if (side === 'column') {
    return `minmax(0, 1fr) clamp(${SIDE_MIN_PX}px, ${SIDE_SHARE * 100}vw, ${SIDE_MAX_PX}px)`
  }
  return side === 'rail' ? `minmax(0, 1fr) ${RAIL_PX}px` : 'minmax(0, 1fr)'
}

/** The least height a sheet that stops above your board gets from the opponents' area and the shared strip, in px. */
export const SIDE_PANEL_MIN_PX = 300

/** The least height an open rail panel or sheet keeps while a prompt waits in the shared strip, in px: the tabs and a row. */
export const SIDE_DECISION_MIN_PX = 144

/**
 * The `grid-row` of the overlay that holds an open rail panel or sheet, by the lines of the main area's five rows (bar,
 * opponents, shared strip, own board, hand and controls). Never the bar, whose menu stays in reach, and never the
 * hand and controls row, so the hand and the pass control stay pressable. A rail's panel reaches down over your board
 * (your life is in the pod beside it); a sheet stops above your board while the opponents' area and the strip give it
 * `SIDE_PANEL_MIN_PX`, so your life stays in view, and else takes your board as well. While a prompt waits in the strip
 * (`decisionInStrip`) either takes the opponents' row only, which `SidePanelMount` shortens by the prompt sheet's
 * overhang, so the prompt and the cards it asks you to click stay in view and pressable.
 */
export function sidePanelRows(
  regions: Pick<Regions, 'side' | 'opponents' | 'shared'>,
  decisionInStrip = false,
): string {
  if (decisionInStrip) return '2 / 3'
  const aboveOwn = regions.side === 'sheet' && regions.opponents + regions.shared >= SIDE_PANEL_MIN_PX
  return aboveOwn ? '2 / 4' : '2 / 5'
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
  /** The sides of a fight against a boss, which opens in boss mode. */
  sides?: BossSides | null | undefined
  viewer?: PlayerId | undefined
}

/**
 * Boss mode in a fight against a boss; else grid for 2 to 4 seats, focus for 5 or more or when a grid column would
 * be under `MIN_COLUMN_PX`; the player may override.
 */
export function chooseMode({ seatCount, mainWidth, override, sides, viewer }: ModeInput): TableMode {
  if (override !== undefined) return override
  if (bossSidesFor(sides, viewer) !== null) return 'boss'
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
  /** The seat whose turn it is; in focus mode its tile leads the strip. */
  active?: PlayerId | null | undefined
}

/** `tiles` with `active` moved to the front, the rest as they were; unchanged when `active` is not among them. */
export function activeFirst(tiles: readonly PlayerId[], active: PlayerId | null | undefined): PlayerId[] {
  if (active === null || active === undefined || !tiles.includes(active)) return [...tiles]
  return [active, ...tiles.filter((seat) => seat !== active)]
}

/**
 * Boss mode on a phone: with minions, every seat of the boss's side is a tile and only the focused one (the boss
 * unless a tile was picked) has its board drawn; the boss alone is drawn whole.
 */
export function bossOnPhone(plan: SeatPlan, phone: boolean): SeatPlan {
  if (!phone || plan.shape !== 'boss' || plan.boards.length <= 1) return plan
  const focused = plan.focused ?? plan.boards[0] ?? null
  return { ...plan, tiles: plan.boards, boards: focused === null ? [] : [focused], columns: 1 }
}

/** The height in px of the party's band over your board in boss mode: a tile strip on a phone, else a row of boards. */
export function alliesBandPx(regions: Pick<Regions, 'phone' | 'opponents' | 'own'>, allies: number): number {
  if (allies <= 0) return 0
  if (regions.phone) return ALLY_STRIP_PX
  const room = finite(regions.opponents) + finite(regions.own)
  return Math.round(Math.min(ALLIES_MAX_PX, Math.max(ALLIES_MIN_PX, room * ALLIES_SHARE)))
}

/** The row of the boss's abilities over its side (`h-9`). */
export const BOSS_ABILITIES_PX = 36

/** The boss's board is this many shares of its side's width, beside one share per minion. */
export const BOSS_SPAN = 2

/** A hero's board widened in the party's row takes this many shares of it, beside one share per other hero. */
export const ALLY_WIDE_SPAN = 3

/** The party's tile strip on a phone (`h-14`). */
export const ALLY_STRIP_PX = 56
/** The share of the boards' room the party's row of boards takes, and its bounds in px. */
export const ALLIES_SHARE = 0.34
export const ALLIES_MIN_PX = 120
export const ALLIES_MAX_PX = 280

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
  /** Whether the step ladder (the stop buttons) sits in the top bar; otherwise it is in the dock's "Stops" popover. */
  ladderInBar: boolean
  /** How much the controls dock writes (`dockForm`). */
  dock: DockForm
}

/** Sizes the regions, chooses the mode and arranges the seats. */
export function planTable(input: TableInput): TablePlan {
  const regions = regionSizes(input.viewport)
  const seatCount = wholeSeats(input.seatCount)
  const mode = chooseMode({
    seatCount,
    mainWidth: regions.main.width,
    override: input.mode,
    sides: input.sides,
    viewer: input.viewer,
  })
  const arranged = bossOnPhone(arrangeSeats({ ...input, seatCount, mode }), regions.phone)
  // The strip leads with the seat whose turn it is, so that it is in view where the strip scrolls and one place to look at everywhere.
  const seats = { ...arranged, tiles: activeFirst(arranged.tiles, input.active) }
  const pod: PodPlace = regions.phone ? 'line' : 'aside'
  const tiled = seats.shape === 'focus' || (seats.shape === 'boss' && seats.tiles.length > 0)
  return {
    regions,
    seats,
    pod,
    opponentPod: regions.phone && tiled ? 'none' : pod,
    stopsRow: BREAKPOINT_FROM[regions.breakpoint] >= STOPS_ROW_MIN_WIDTH_PX,
    ladderInBar: BREAKPOINT_FROM[regions.breakpoint] >= LADDER_IN_BAR_MIN_WIDTH_PX,
    dock: dockForm(regions),
  }
}

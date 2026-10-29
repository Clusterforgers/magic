/** Pure height split between the opponents' boards and yours, weighted by the lanes each must hold. */
import { LANE_LIMITS, highestRung, ladderOf, laneHeight } from './laneFit'
import type { LaneCounts, LaneLimits } from './laneFit'
import { OWN_SHARE, POD_COLUMN_PX, SHEET_MAX_VH, ZONE_GAP_PX } from './layout'
import type { Regions, SeatPlan } from './layout'
import type { PlayState } from '@/play/store'
import type { PlayerId } from '@/protocol/generated'
import { battlefieldStacks } from './rowStacks'
import type { StackIds } from './rowStacks'

/** The boxes of the lanes of your board and of each opponent's board drawn. */
export interface BoardCounts {
  own: LaneCounts
  theirs: readonly LaneCounts[]
}

/** The closed stacks of a seat's lanes, one box each, as the board draws them before any stack opens. */
export function seatCounts(state: PlayState, seat: PlayerId, grouped = true): LaneCounts {
  const { plain } = battlefieldStacks(state, seat, grouped)
  const counts: LaneCounts = {
    lands: plain.lands.length,
    creatures: plain.creatures.length,
    other: plain.other.length,
  }
  const deepest = (row: readonly StackIds[]): number =>
    Math.max(0, ...row.map((stack) => stack.tucked?.length ?? 0))
  const tucked = {
    lands: deepest(plain.lands),
    creatures: deepest(plain.creatures),
    other: deepest(plain.other),
  }
  return tucked.lands + tucked.creatures + tucked.other === 0 ? counts : { ...counts, tucked }
}

/** The counts of the viewer's board and of every opponent's board of a plan. */
export function boardCounts(state: PlayState, seats: SeatPlan, grouped = true): BoardCounts {
  return {
    own: seatCounts(state, seats.viewer, grouped),
    theirs: seats.boards.map((seat) => seatCounts(state, seat, grouped)),
  }
}

/** The most of a prompt sheet's overhang the split keeps free above the strip: two lines of a prompt. */
export const SHEET_RESERVE_MAX_PX = 64

/** The room the split keeps for a sheet standing `overhang` px above the strip: in steps of 8 px, at most `SHEET_RESERVE_MAX_PX`. */
export function sheetReserve(overhang: number): number {
  return Math.min(SHEET_RESERVE_MAX_PX, Math.ceil(finite(overhang) / 8) * 8)
}

/**
 * `regions` with the board rows split for these boards (`splitBoards` over `boardFrames`); `reserve` px of the
 * opponents' row are kept for the prompt sheet (`sheetReserve`), so a two-line prompt shrinks both boards a little
 * instead of the opponents' a lot.
 */
export function splitRegions(regions: Regions, seats: SeatPlan, counts: BoardCounts, reserve = 0): Regions {
  const frames = boardFrames(regions, seats)
  const split = splitBoards({
    room: regions.opponents + regions.own,
    ownChrome: frames.own.chrome,
    theirChrome: frames.theirs.chrome + finite(reserve),
    ownWidth: frames.own.width,
    theirWidth: frames.theirs.width,
    own: counts.own,
    theirs: counts.theirs,
  })
  return withSplit(regions, split)
}

/** An opponent's cards are this share of yours: their board shrinks first. */
export const THEIR_SCALE = 0.8
/** Neither row gets less than this share of the room both share, so an empty board keeps a place for a prompt sheet. */
export const MIN_ROW_SHARE = 0.25
/** The padding of a battlefield's zone around its lanes (`p-1`). */
const ZONE_PAD_PX = 4
/** The one-line pod over a board of the columns shape, and the tile strip of focus mode (`h-14`). */
const LINE_POD_PX = 40
const TILE_STRIP_PX = 56

/** What `splitBoards` needs: the room of both rows and the boards inside them. */
export interface SplitInput {
  /** The height the opponents' row and your row share in px. */
  room: number
  /** What each row spends on other things than its board's lanes: padding, a pod line, the tile strip. */
  ownChrome: number
  theirChrome: number
  /** The width of the lanes of your board and of each opponent's board in px. */
  ownWidth: number
  theirWidth: number
  /** The boxes of each lane of your board and of each opponent's board drawn. */
  own: LaneCounts
  theirs: readonly LaneCounts[]
}

/** The heights of both rows and the card widths they were chosen for. */
export interface BoardSplit {
  opponents: number
  own: number
  /** Your creatures' card width the split holds, and the opponents'; 0 where nothing fits or no board needs room. */
  ownCard: number
  theirCard: number
}

const finite = (value: number): number => (Number.isFinite(value) ? Math.max(0, value) : 0)

/** The default split of `room`: your row `OWN_SHARE` times as tall as the opponents'. */
export function shareSplit(room: number): BoardSplit {
  const opponents = finite(room) / (1 + OWN_SHARE)
  return { opponents, own: finite(room) - opponents, ownCard: 0, theirCard: 0 }
}

/**
 * The largest card `w` of the ladder at which your lanes at `w` and every opponent's at `THEIR_SCALE` of it fit the
 * room together; the opponents' row gets what its lanes need and your row the rest, each at least `MIN_ROW_SHARE` of
 * the room. What is left lifts the opponents' cards further, up to `w`. Without a fit at the floor, or with no
 * permanent anywhere, the room splits by `OWN_SHARE`.
 */
export function splitBoards(input: SplitInput, limits: LaneLimits = LANE_LIMITS): BoardSplit {
  const room = finite(input.room)
  const { at, rungs } = ladderOf(limits)
  const least = room * MIN_ROW_SHARE
  const ownRow = (card: number): number =>
    Math.max(least, laneHeight(input.ownWidth, input.own, card, limits) + finite(input.ownChrome))
  const theirNeed = (card: number): number =>
    Math.max(0, ...input.theirs.map((counts) => laneHeight(input.theirWidth, counts, card, limits)))
  const theirRow = (card: number): number => Math.max(least, theirNeed(card) + finite(input.theirChrome))
  const theirCard = (card: number): number => Math.floor(card * THEIR_SCALE)
  const fits = (rung: number): boolean => ownRow(at(rung)) + theirRow(theirCard(at(rung))) <= room
  const empty = laneHeight(input.ownWidth, input.own, at(0), limits) + theirNeed(theirCard(at(0)))
  if (!fits(0) || empty === 0) return shareSplit(room)
  const card = at(highestRung(rungs, fits))
  const spare = room - ownRow(card)
  let theirs = theirCard(card)
  while (theirs + limits.step <= card && theirRow(theirs + limits.step) <= spare) theirs += limits.step
  const opponents = theirRow(theirs)
  return { opponents, own: room - opponents, ownCard: card, theirCard: theirs }
}

/** A board's lanes: their width, and the px its row spends on other things. */
export interface BoardFrame {
  width: number
  chrome: number
}

/** The width of a board's lanes and what its row spends besides them, for each seat shape. */
export function boardFrames(regions: Regions, seats: SeatPlan): { own: BoardFrame; theirs: BoardFrame } {
  const main = regions.main.width
  const aside = main - 2 * ZONE_GAP_PX - POD_COLUMN_PX - ZONE_GAP_PX - 2 * ZONE_PAD_PX
  const full = main - 2 * ZONE_GAP_PX - 2 * ZONE_PAD_PX
  const pads = 2 * ZONE_GAP_PX + 2 * ZONE_PAD_PX
  const ownPod = regions.phone ? LINE_POD_PX + ZONE_GAP_PX : 0
  const own = { width: regions.phone ? full : aside, chrome: pads + ownPod }
  if (seats.shape === 'columns') {
    const column = (main - 2 * ZONE_GAP_PX - (seats.columns - 1) * ZONE_GAP_PX) / seats.columns
    return { own, theirs: { width: column - 2 * ZONE_PAD_PX, chrome: pads + LINE_POD_PX + ZONE_GAP_PX } }
  }
  const strip = seats.shape === 'focus' ? TILE_STRIP_PX + ZONE_GAP_PX : 0
  return { own, theirs: { width: regions.phone ? full : aside, chrome: pads + strip } }
}

/** `regions` with the rows of a split, and the sheet's most kept inside the opponents' row and the strip. */
export function withSplit(regions: Regions, split: BoardSplit): Regions {
  return {
    ...regions,
    opponents: split.opponents,
    own: split.own,
    sheet: Math.min((regions.main.height * SHEET_MAX_VH) / 100, regions.shared + split.opponents),
  }
}

/** Pure height split between the opponents' boards and yours, weighted by the lanes each must hold. */
import { LANE_LIMITS, highestRung, ladderOf, laneHeight } from './laneFit'
import type { LaneCounts, LaneLimits } from './laneFit'
import { POD_CRAMPED_PX, POD_PICTURES_PX } from './cornerFit'
import {
  BOSS_ABILITIES_PX,
  BOSS_SPAN,
  OWN_SHARE,
  POD_COLUMN_PX,
  ZONE_GAP_PX,
  alliesBandPx,
  sheetMost,
} from './layout'
import type { Regions, SeatPlan } from './layout'
import type { PlayState } from '@/play/store'
import type { PlayerId } from '@/protocol/generated'
import { bossSplitOf } from './bossRows'
import type { BossSplit } from './bossSplit'
import { GROW_SHARE, padded } from './holdFit'
import { handIdle } from './handIdle'
import { handSeat } from './selectors'
import { battlefieldStacks } from './rowStacks'
import type { StackIds } from './rowStacks'

/** The boxes of the lanes of your board and of each opponent's board drawn. */
export interface BoardCounts {
  own: LaneCounts
  theirs: readonly LaneCounts[]
  /** Boss mode: the other heroes' boards, in the party's order; the boards of the boss's side are `theirs`. */
  allies?: readonly LaneCounts[]
  /** Boss mode: your hand has a card to play, so an empty board of yours keeps room (`handIdle`). */
  ownReady?: boolean
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
  const counts: BoardCounts = {
    own: seatCounts(state, seats.viewer, grouped),
    theirs: seats.boards.map((seat) => seatCounts(state, seat, grouped)),
  }
  if (seats.shape !== 'boss') return counts
  return {
    ...counts,
    allies: seats.allies.map((seat) => seatCounts(state, seat, grouped)),
    ownReady: !handIdle(state, handSeat(state, seats.viewer)),
  }
}

/**
 * Whether the boards of a plan carry a lands chip (`LandFoldContext`): every board of a grid of opponents and every
 * board of a phone, yours too, where the table folds lands. The boards fold their lands into it, and hold no lands
 * lane, unless the viewer shows lands (`landPrefs.ts`).
 */
export const foldsLands = (seats: SeatPlan, fold: boolean, phone = false): boolean =>
  fold && (seats.shape === 'columns' || (phone && seats.shape !== 'boss'))

/**
 * Whether the opponents' boards of a plan fold their lands until the viewer chooses (as a phone's do): a grid of more
 * opponents than one row of frames holds, whose boards are too low for a lands lane beside their other permanents.
 */
export const crowdFoldsLands = (seats: SeatPlan): boolean =>
  seats.shape === 'columns' && seats.boards.length > seats.columns

/** The most of a prompt sheet's overhang the split keeps free above the strip: two lines of a prompt. */
export const SHEET_RESERVE_MAX_PX = 64

/** The room the split keeps for a sheet standing `overhang` px above the strip: in steps of 8 px, at most `SHEET_RESERVE_MAX_PX`. */
export function sheetReserve(overhang: number): number {
  return Math.min(SHEET_RESERVE_MAX_PX, Math.ceil(finite(overhang) / 8) * 8)
}

/**
 * What the opponents' row spends besides its lanes: the estimate of `boardFrames`, or more where their battlefields
 * begin `head` px down the row (a pod line wrapped to several lines), with the zone's padding and the row's bottom gap.
 */
export function theirChromeOf(estimate: number, head: number): number {
  const measured = finite(head) === 0 ? 0 : Math.ceil((finite(head) + 2 * ZONE_PAD_PX + ZONE_GAP_PX) / 8) * 8
  return Math.max(finite(estimate), measured)
}

/** The least of the opponents' boards their row keeps below their pods, and a prompt sheet leaves off a phone: a row of their smallest cards. */
export const BOARD_KEEP_PX = 64

/**
 * The least height of the opponents' row where an opponent's pod stands as a column beside the board (off a phone,
 * not in columns): the column holds the pod's tight blocks and the least picture (`POD_PICTURES_PX`) above a sheet
 * standing `overhang` px over the row, so it neither scrolls nor folds its chips under the corner; with a `column` of
 * `POD_CRAMPED_PX`, the least that holds the tight blocks with the corner's chips. 0 elsewhere.
 */
export function podColumnFloor(
  regions: Regions,
  seats: SeatPlan,
  overhang: number,
  column = POD_PICTURES_PX,
): number {
  if (regions.phone || seats.shape === 'columns') return 0
  return boardFrames(regions, seats).theirs.chrome + column + Math.ceil(finite(overhang) / 8) * 8
}

/**
 * `regions` with the board rows split for these boards (`splitBoards` over `boardFrames`); `reserve` px of the
 * opponents' row are kept for the prompt sheet (`sheetReserve`), so a two-line prompt shrinks both boards a little
 * instead of the opponents' a lot. Their battlefields begin `head` px down their row (`theirChromeOf`), and the sheet
 * leaves them `BOARD_KEEP_PX` below that. The sheet wants `overhang` px over the row, which a pod column keeps free
 * below itself (`podColumnFloor`).
 */
export function splitRegions(
  regions: Regions,
  seats: SeatPlan,
  counts: BoardCounts,
  reserve = 0,
  head = 0,
  overhang = reserve,
  held: HeldRows | null = null,
): Regions {
  // The band's room above the strip is kept in every split, prompt or none, so no prompt moves a board.
  reserve = Math.max(finite(reserve), finite(regions.sheetRoom))
  overhang = Math.max(finite(overhang), finite(regions.sheetRoom))
  // Boss mode splits three rows by what each shows (`splitBoss`); a phone's tiles and a board-less call use the two rows.
  if (seats.shape === 'boss' && !regions.phone && seats.tiles.length === 0 && counts.allies !== undefined) {
    const { split, shares, chrome } = bossSplitOf(
      regions,
      seats,
      { own: counts.own, theirs: counts.theirs, allies: counts.allies, ownReady: counts.ownReady ?? true },
      finite(reserve),
      finite(overhang),
      held?.band === undefined || held.bossCard === undefined || held.heroCard === undefined
        ? null
        : {
            opponents: held.opponents,
            own: held.own,
            band: held.band,
            bossCard: held.bossCard,
            heroCard: held.heroCard,
          },
    )
    const party = { band: split.band, bossCard: split.bossCard, heroCard: split.heroCard, shares }
    const rows = {
      opponents: split.opponents,
      own: split.own,
      ownCard: split.heroCard,
      theirCard: split.bossCard,
    }
    return { ...withSplit(regions, rows, chrome - reserve + BOARD_KEEP_PX), party }
  }
  const frames = boardFrames(regions, seats)
  const theirChrome = theirChromeOf(frames.theirs.chrome, head)
  const split = holdSplit(
    {
      room: regions.opponents + regions.own,
      ownChrome: frames.own.chrome,
      theirChrome: theirChrome + finite(reserve),
      theirFloor: podColumnFloor(regions, seats, overhang),
      theirFloorLeast: podColumnFloor(regions, seats, overhang, POD_CRAMPED_PX),
      theirRows: frames.theirs.rows,
      theirRowChrome: theirChrome,
      theirLead: theirChrome - frames.theirs.chrome,
      theirScale: seats.shape === 'columns' ? GRID_THEIR_SCALE : THEIR_SCALE,
      ownCrowd: seats.shape === 'columns' && !regions.phone ? OWN_CROWD_PX : 0,
      ownWidth: frames.own.width,
      theirWidth: frames.theirs.width,
      own: counts.own,
      theirs: counts.theirs,
    },
    held,
  )
  if (!regions.phone) return withSplit(regions, split, theirChrome + BOARD_KEEP_PX)
  // A phone's sheet lies in the band's room, which the split counts in the opponents' chrome; a sheet of cards leaves
  // their pods, tiles and a boss's bar and pod (`theirChrome`) clear.
  return withSplit(regions, split, theirChrome)
}

/** The least width in px of your creatures for which the split gives an opponent's pod column its floor. */
export const FLOOR_OWN_CARD_PX = 100

/**
 * Your creatures' least width in px at a grid of opponents: a crowded table gives your row the room for it before the
 * opponents' boards grow past their floor, since you play from your board and each opponent's enlarges from its pod.
 */
export const OWN_CROWD_PX = 60

/** An opponent's cards are this share of yours: their board shrinks first. */
export const THEIR_SCALE = 0.8
/** In a grid of opponents their cards are as large as yours: three boards side by side would otherwise fall unreadable. */
export const GRID_THEIR_SCALE = 0.85
/** Neither row gets less than this share of the room both share, so an empty board keeps a place for a prompt sheet. */
export const MIN_ROW_SHARE = 0.25
/** The padding of a battlefield's zone around its lanes (`p-1`). */
export const ZONE_PAD_PX = 4
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
  /** The height an opponent's pod column asks of the opponents' row (`podColumnFloor`). */
  theirFloor?: number
  /** The least of it, where the whole floor would take your creatures under `FLOOR_OWN_CARD_PX` (chips in place of pictures). */
  theirFloorLeast?: number
  /** The rows of boards the opponents' area stacks (a grid of more opponents than columns); 1 when left out. */
  theirRows?: number
  /** An opponent's cards as a share of yours; `THEIR_SCALE` when left out. */
  theirScale?: number
  /** What each row of opponents' boards after the first spends besides its lanes (its pod line and frame). */
  theirRowChrome?: number
  /** What the opponents' row spends above their boards beyond `theirChrome`'s estimate (seats that left, a wrapped pod line), which its least share does not count; 0 when left out. */
  theirLead?: number
  /** Your creatures' least width in px before the opponents' boards grow past their floor (`OWN_CROWD_PX`); 0 when left out. */
  ownCrowd?: number
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
 * the room. What is left lifts the opponents' cards further, up to `w`, and what even that leaves is shared evenly
 * by both rows. Without a fit at the floor, or with no
 * permanent anywhere, the room splits by `OWN_SHARE`, the opponents' row keeping its chrome and `BOARD_KEEP_PX`, and
 * as much of what their lanes need at the floor as your row leaves above its own floor (at least `MIN_ROW_SHARE`). The opponents' row is at least `theirFloor` where your creatures keep
 * `FLOOR_OWN_CARD_PX`; elsewhere only room your lanes leave lifts it towards that.
 */
export function splitBoards(input: SplitInput, limits: LaneLimits = LANE_LIMITS): BoardSplit {
  const room = finite(input.room)
  const { at, rungs } = ladderOf(limits)
  const least = room * MIN_ROW_SHARE
  const ownRow = (card: number): number =>
    Math.max(least, laneHeight(input.ownWidth, input.own, card, limits) + finite(input.ownChrome))
  const theirNeed = (card: number): number =>
    Math.max(0, ...input.theirs.map((counts) => laneHeight(input.theirWidth, counts, card, limits)))
  const rows = Math.max(1, Math.floor(finite(input.theirRows ?? 1)))
  const theirRow = (card: number): number =>
    Math.max(
      least + finite(input.theirLead ?? 0),
      rows * theirNeed(card) + finite(input.theirChrome) + (rows - 1) * finite(input.theirRowChrome ?? 0),
    )
  const theirCard = (card: number): number => Math.floor(card * finite(input.theirScale ?? THEIR_SCALE))
  const fits = (rung: number): boolean => ownRow(at(rung)) + theirRow(theirCard(at(rung))) <= room
  const empty = laneHeight(input.ownWidth, input.own, at(0), limits) + theirNeed(theirCard(at(0)))
  const crowd = finite(input.ownCrowd ?? 0)
  // A crowd: your creatures would fall under `ownCrowd` with every board fitting together.
  const crowdedOut = crowd > 0 && empty > 0 && fits(0) && at(highestRung(rungs, fits)) < crowd
  if (!fits(0) || empty === 0 || crowdedOut) {
    const share = shareSplit(room)
    if (input.theirs.length === 0) return share
    // Opponents that cannot fit even at the floor take what they need there before your row grows past its least,
    // which in a crowd holds your creatures at `ownCrowd`.
    const ownLeast = empty === 0 ? least : Math.max(least, ownRow(Math.max(at(0), crowd)))
    const crowded = empty === 0 ? 0 : Math.min(room - ownLeast, theirRow(at(0)))
    const keep = Math.max(crowded, Math.min(room - least, finite(input.theirChrome) + BOARD_KEEP_PX))
    if (crowd > 0 && empty > 0) {
      const opponents = Math.max(keep, Math.min(share.opponents, room - ownLeast))
      return { ...share, opponents, own: room - opponents }
    }
    return share.opponents >= keep ? share : { ...share, opponents: keep, own: room - keep }
  }
  // The opponents' row is lifted to the floor where your creatures stay `FLOOR_OWN_CARD_PX` wide; otherwise it takes
  // only what your lanes leave.
  // Where the pod's full floor would take your creatures under `FLOOR_OWN_CARD_PX`, its least floor is tried.
  const liftTo = (asked: number | undefined): { floor: number; lifted: number } => {
    const floor = Math.min(room - least, finite(asked ?? 0))
    const floored = (rung: number): boolean =>
      ownRow(at(rung)) + Math.max(floor, theirRow(theirCard(at(rung)))) <= room
    return { floor, lifted: floor > 0 && floored(0) ? at(highestRung(rungs, floored)) : 0 }
  }
  const full = liftTo(input.theirFloor)
  const { floor, lifted } =
    full.lifted >= FLOOR_OWN_CARD_PX || input.theirFloorLeast === undefined ? full : liftTo(input.theirFloorLeast)
  const card = lifted >= FLOOR_OWN_CARD_PX ? lifted : at(highestRung(rungs, fits))
  const spare = room - ownRow(card)
  let theirs = theirCard(card)
  while (theirs + limits.step <= card && theirRow(theirs + limits.step) <= spare) theirs += limits.step
  const held = Math.max(theirRow(theirs), Math.min(floor, spare))
  // Room your cards cannot use even all of it (the next card is too wide for a band more, or the largest) is shared
  // evenly, so a nearly empty board keeps its place and the strip stays between the boards instead of riding up.
  const grows = card + limits.step <= limits.maxWidth && ownRow(card + limits.step) <= room - held
  const opponents = grows ? held : held + Math.max(0, spare - held) / 2
  return { opponents, own: room - opponents, ownCard: card, theirCard: theirs }
}

/** A board's lanes: their width, and the px its row spends on other things. */
export interface BoardFrame {
  width: number
  chrome: number
  /** The rows of boards stacked in the row (a grid of opponents); 1 when left out. */
  rows?: number
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
    const rows = Math.max(1, Math.ceil(seats.boards.length / seats.columns))
    const chrome = pads + LINE_POD_PX + ZONE_GAP_PX
    return { own, theirs: { width: column - 2 * ZONE_PAD_PX, chrome, rows } }
  }
  if (seats.shape === 'boss') return bossFrames(regions, seats, own, { aside, full, pads })
  const strip = seats.shape === 'focus' ? TILE_STRIP_PX + ZONE_GAP_PX : 0
  return { own, theirs: { width: regions.phone ? full : aside, chrome: pads + strip } }
}

/**
 * The frames of boss mode: your row also holds the party's band over your board (`alliesBandPx`); the boss's board
 * is the wide column of its side (`BOSS_SPAN` shares beside one per minion), or the one board under the side's tiles
 * on a phone.
 */
function bossFrames(
  regions: Regions,
  seats: SeatPlan,
  own: BoardFrame,
  widths: { aside: number; full: number; pads: number },
): { own: BoardFrame; theirs: BoardFrame } {
  const band = alliesBandPx(regions, seats.allies.length)
  const party = { ...own, chrome: own.chrome + (band > 0 ? band + ZONE_GAP_PX : 0) }
  const minions = regions.phone ? 0 : Math.max(0, seats.boards.length - 1)
  const strip = (seats.tiles.length > 0 ? TILE_STRIP_PX + ZONE_GAP_PX : 0) + BOSS_ABILITIES_PX + ZONE_GAP_PX
  if (minions === 0) {
    return {
      own: party,
      theirs: { width: regions.phone ? widths.full : widths.aside, chrome: widths.pads + strip },
    }
  }
  const room = regions.main.width - 2 * ZONE_GAP_PX - minions * ZONE_GAP_PX
  const column = (room * BOSS_SPAN) / (BOSS_SPAN + minions)
  return {
    own: party,
    theirs: {
      width: column - POD_COLUMN_PX - ZONE_GAP_PX - 2 * ZONE_PAD_PX,
      chrome: widths.pads + BOSS_ABILITIES_PX + ZONE_GAP_PX,
    },
  }
}

/**
 * `regions` with the rows of a split and the sheet's most: the band, whose room the split keeps free of cards, and for a
 * prompt that lays out cards up over the opponents' boards to leave `keep` px of their row (their pods and a row of
 * cards; on a phone at least `SHEET_KEEP_PX`; `sheetMost`).
 */
export function withSplit(regions: Regions, split: BoardSplit, keep = 0): Regions {
  const most = sheetMost(regions.main.height, regions.shared, split.opponents, regions.phone, keep)
  // The band is the strip with the room kept above it (the ticker's line, a narrow window's second line), whatever the split leaves.
  const sheet = regions.shared + finite(regions.sheetRoom)
  return { ...regions, opponents: split.opponents, own: split.own, sheet, sheetWide: Math.max(sheet, most) }
}

/** Without a fit a held split keeps its rows while the fresh one lies within this share of the room of them. */
export const SPLIT_DRIFT_SHARE = 0.08

/** The rows a split drew last, which the next split holds where they still serve (`holdSplit`). */
export type HeldRows = Pick<BoardSplit, 'opponents' | 'own'> &
  Partial<Pick<BossSplit, 'band' | 'bossCard' | 'heroCard'>>

/**
 * `splitBoards`, holding the rows of `held` while the room is the same and both rows still fit their boards at the
 * floor, unless a fresh split gives a board a card `GROW_SHARE` larger even with one permanent more in every lane
 * (clear room, so the next permanent does not split back): one permanent more or less then moves no row. Rows that no
 * longer fit give way to the fresh split, or, where that has no fit either, are held while it lies within
 * `SPLIT_DRIFT_SHARE` of the room of them.
 */
export function holdSplit(
  input: SplitInput,
  held: HeldRows | null,
  limits: LaneLimits = LANE_LIMITS,
): BoardSplit {
  const fresh = splitBoards(input, limits)
  const room = finite(input.room)
  if (held === null || Math.abs(held.opponents + held.own - room) > 0.5) return fresh
  const least = room * MIN_ROW_SHARE
  const kept = { ...fresh, opponents: held.opponents, own: held.own }
  const { at, rungs } = ladderOf(limits)
  const rows = Math.max(1, Math.floor(finite(input.theirRows ?? 1)))
  const ownFits = (rung: number): boolean =>
    laneHeight(input.ownWidth, input.own, at(rung), limits) + finite(input.ownChrome) <= held.own
  const theirNeed = (rung: number): number =>
    Math.max(0, ...input.theirs.map((counts) => laneHeight(input.theirWidth, counts, at(rung), limits)))
  const theirFits = (rung: number): boolean =>
    rows * theirNeed(rung) + finite(input.theirChrome) + (rows - 1) * finite(input.theirRowChrome ?? 0) <=
    held.opponents
  const floored = held.opponents >= Math.min(fresh.opponents, finite(input.theirFloor ?? 0))
  if (!ownFits(0) || !theirFits(0) || !floored || held.own < least || held.opponents < least) {
    const near = Math.abs(fresh.opponents - held.opponents) <= room * SPLIT_DRIFT_SHARE
    return fresh.ownCard === 0 && near && held.own >= least && held.opponents >= least ? kept : fresh
  }
  const ownCard = at(highestRung(rungs, ownFits))
  const theirCard = at(highestRung(rungs, theirFits))
  const roomy = splitBoards({ ...input, own: padded(input.own), theirs: input.theirs.map(padded) }, limits)
  const better = (card: number, now: number, more: number): boolean =>
    card * (1 + GROW_SHARE) < now && card * (1 + GROW_SHARE) < more
  if (better(ownCard, fresh.ownCard, roomy.ownCard) || better(theirCard, fresh.theirCard, roomy.theirCard))
    return fresh
  return { ...kept, ownCard, theirCard }
}

/** Pure split of the table's height in boss mode: the boss's side, the other heroes' row and your board, each by what it draws. */
import { ALLY_STRIP_PX } from './layout'
import { GROW_SHARE, padded } from './holdFit'
import { LANE_LIMITS, highestRung, ladderOf, laneHeight } from './laneFit'
import type { LaneCounts, LaneLimits } from './laneFit'

/** The most of the boards' room the boss's side takes. */
export const BOSS_SIDE_MOST = 0.45
/** The boss's cards are this share of the heroes': the party plays from its boards, so its cards are the larger. */
export const BOSS_SCALE = 0.85
/** An ally's column takes at least this many shares of the party's row, as for a board of that many permanents. */
export const ALLY_MIN_SHARE = 4
/** The boss's cards are held at this width in px, at the least, while the heroes' cards grow. */
export const BOSS_CARD_FLOOR_PX = 60
/**
 * The boss's cards are held at this share of the heroes' at the least while the heroes' grow (up to `BOSS_SCALE` and
 * `BOSS_SIDE_MOST`), so the boss's board stays readable beside theirs; a step of the card ladder under it leaves 0.6.
 */
export const BOSS_FAIR_SHARE = 0.66
/** The party's row of boards is shown from this height in px; under it the heroes are a strip of tiles. */
export const ALLY_ROW_MIN_PX = 120

/** The height of the party's row when no ally has a permanent: the tiles in their frames. */
export const ALLY_LINE_PX = 68
/** The height of your row when you have no permanent and no card to play: your pod as a line. */
export const OWN_LINE_PX = 72
/** What an empty board of yours that still has a hand keeps room for: a few lands, as on any early turn. */
export const OWN_READY_COUNTS: LaneCounts = { lands: 3, creatures: 1, other: 0 }
/** What the boss's board keeps room for while its side has no permanent: as yours with a hand, so its first ones move no row. */
export const BOSS_READY_COUNTS: LaneCounts = OWN_READY_COUNTS
/** The least height of a pod column beside a board that has permanents, in px. */
export const POD_MIN_PX = 104

/** A board of the split: the width of its lanes, its boxes, and what its frame draws above the lanes in px. */
export interface BossBoard {
  width: number
  counts: LaneCounts
  /** What the board's frame spends above its lanes (a minion's pod line); 0 when left out. */
  head?: number
}

/** What `splitBoss` needs: the room of all three and the boards inside them. */
export interface BossSplitInput {
  /** The height the boss's side and the party's rows share in px. */
  room: number
  /** The boss's board first, then each minion's. */
  boss: readonly BossBoard[]
  /** What the boss's row spends besides its lanes: the abilities, frames and padding. */
  bossChrome: number
  /** The least height of the boss's row where it has permanents: the pod column beside the board. */
  bossFloor: number
  /** The other heroes' boards; none for a party of one. */
  allies: readonly BossBoard[]
  /** What the party's row spends besides its lanes: tile, frames and padding. */
  allyChrome: number
  /** Your board. */
  own: BossBoard
  /** What your row spends besides your lanes: padding. */
  ownChrome: number
  /** Your hand holds cards that will land on your board, so an empty board keeps room (`OWN_READY_COUNTS`). */
  ownReady: boolean
  /** The least height of your row where it has permanents: the pod column beside the board. */
  ownFloor: number
  /** The space between the allies' row and your board. */
  gap: number
}

/** The heights of the three and the card widths they were chosen for. */
export interface BossSplit {
  /** The boss's side. */
  opponents: number
  /** The party: the allies' row, the gap and your board. */
  own: number
  /** The allies' row (`ALLY_STRIP_PX` where the heroes are a strip of tiles); 0 without allies. */
  band: number
  /** The widest card of the boss's side and of the heroes, in px; 0 where nothing fits or no board has a permanent. */
  bossCard: number
  heroCard: number
}

const finite = (value: number): number => (Number.isFinite(value) ? Math.max(0, value) : 0)

/** The boxes of a board's lanes, the measure of what it shows. */
export const permanentsIn = (counts: LaneCounts): number => counts.lands + counts.creatures + counts.other

/** The shares of the party's row for allies with these boards: by what each shows, at least `ALLY_MIN_SHARE`. */
export function allyShares(allies: readonly LaneCounts[]): number[] {
  return allies.map((counts) => Math.max(ALLY_MIN_SHARE, permanentsIn(counts)))
}

/**
 * Splits `room` between the boss's side, the allies' row and your board so that every board draws as large a card as
 * the room holds: the heroes' cards are the largest rung of the ladder at which the heroes' rows fit with the boss's row
 * at `BOSS_CARD_FLOOR_PX` cards; the boss then takes what is left, up to `BOSS_SCALE` of the heroes' card and
 * `BOSS_SIDE_MOST` of the room. The boss's side without permanents keeps the room of `BOSS_READY_COUNTS`; an ally's
 * board or yours without permanents takes one line (`ALLY_LINE_PX`, `OWN_LINE_PX`), and a hand that will fill an empty
 * board of yours keeps your row. What is
 * left goes to the allies' row and yours in proportion to what they need. Without a fit at the floor the boss takes
 * what it needs there up to its cap, your row what it needs there, and the allies the rest: a strip of tiles where low.
 */
export function splitBoss(input: BossSplitInput, limits: LaneLimits = LANE_LIMITS): BossSplit {
  const room = finite(input.room)
  const { at, rungs } = ladderOf(limits)
  const cap = room * BOSS_SIDE_MOST
  const need = (boards: readonly BossBoard[], card: number): number =>
    Math.max(
      0,
      ...boards.map((board) => finite(board.head ?? 0) + laneHeight(board.width, board.counts, card, limits)),
    )
  const bossShows = input.boss.some((board) => permanentsIn(board.counts) > 0)
  const allyShows = input.allies.some((board) => permanentsIn(board.counts) > 0)
  const ownShows = permanentsIn(input.own.counts) > 0
  const ownKeeps = ownShows || input.ownReady
  const ownBoard: BossBoard = ownShows ? input.own : { ...input.own, counts: OWN_READY_COUNTS }
  const gap = input.allies.length === 0 ? 0 : finite(input.gap)
  const bossCard = (card: number): number => Math.floor(card * BOSS_SCALE)
  // The boss's board keeps at least the room of a few permanents, empty too, so its first ones move no row.
  const first = input.boss[0]
  const bossBoards =
    first === undefined ? input.boss : [...input.boss, { ...first, counts: BOSS_READY_COUNTS }]
  const bossRow = (card: number): number =>
    Math.min(cap, Math.max(finite(input.bossFloor), finite(input.bossChrome) + need(bossBoards, card)))
  const allyRow = (card: number): number =>
    input.allies.length === 0
      ? 0
      : allyShows
        ? finite(input.allyChrome) + need(input.allies, card)
        : ALLY_LINE_PX
  const ownRow = (card: number): number =>
    ownKeeps
      ? Math.max(finite(input.ownFloor), finite(input.ownChrome) + need([ownBoard], card))
      : OWN_LINE_PX
  const heroes = (rung: number): number => allyRow(at(rung)) + gap + ownRow(at(rung))
  const floor = at(0)
  // The heroes grow first; the boss's row is held meanwhile at the least that draws its fair share of their card, and
  // never under the ladder's floor, the least the boss is then drawn at.
  const bossLeast = (card: number): number =>
    Math.max(
      floor,
      Math.min(bossCard(card), Math.max(BOSS_CARD_FLOOR_PX, Math.floor(card * BOSS_FAIR_SHARE))),
    )
  const total = (rung: number): number => bossRow(bossLeast(at(rung))) + heroes(rung)
  // Nothing fits at the floor: each row takes its share of what it needs there.
  if (!(total(0) <= room)) {
    return squeezed(room, [bossRow(floor), allyRow(floor), ownRow(floor)], cap, input, allyShows, gap)
  }
  const rung = highestRung(rungs, (each) => total(each) <= room)
  const card = at(rung)
  // What the heroes leave goes to the boss, up to its cap and its share of the heroes' card.
  const budget = Math.min(cap, room - heroes(rung))
  const step = Math.max(1, at(1) - at(0))
  const most = Math.max(0, Math.min(rungs, Math.floor((bossCard(card) - floor) / step)))
  const bossRung = highestRung(most, (each) => bossRow(at(each)) <= budget)
  const boss = bossRow(at(bossRung))
  const allies = allyRow(card)
  const own = ownRow(card)
  const spare = room - (boss + allies + gap + own)
  // Only a row that draws permanents grows; a line stays a line and your row takes the rest.
  const wantA = allyShows ? allies : 0
  const wantO = ownKeeps ? own : 0
  const band =
    input.allies.length === 0 ? 0 : wantA + wantO === 0 ? allies : allies + (spare * wantA) / (wantA + wantO)
  return {
    opponents: boss,
    own: room - boss,
    band,
    bossCard: bossShows ? at(bossRung) : 0,
    heroCard: anyShown(bossShows, allyShows, ownShows) ? card : 0,
  }
}

const anyShown = (...shown: boolean[]): boolean => shown.some(Boolean)

/** The room split by what each row needs at the floor, the boss's at most `cap`; the allies a strip of tiles where their row is low. */
function squeezed(
  room: number,
  needs: readonly [number, number, number],
  cap: number,
  input: BossSplitInput,
  allyShows: boolean,
  gap: number,
): BossSplit {
  const [boss, allies, own] = needs
  const opponents = Math.min(cap, boss)
  const party = Math.max(0, room - opponents)
  const rest = Math.max(0, party - gap)
  // Your board is where you play: it keeps what it needs at the floor first, and the allies' row takes the rest.
  const least = allyShows ? ALLY_ROW_MIN_PX : ALLY_LINE_PX
  const scaled = allies === 0 ? 0 : rest - Math.min(own, Math.max(0, rest - least))
  const low = scaled < least
  const band = input.allies.length === 0 ? 0 : low ? Math.min(ALLY_STRIP_PX, rest) : scaled
  return { opponents, own: party, band, bossCard: 0, heroCard: 0 }
}

/**
 * `splitBoss`, holding the rows of `held` while the room is the same, every board still fits its row at the held
 * cards, and no fresh split gives the heroes or the boss a card `GROW_SHARE` larger even with one permanent more in
 * every lane: one permanent more or less then moves no row. A split without cards (empty boards, nothing fits) is
 * never held.
 */
export function holdBoss(
  input: BossSplitInput,
  held: BossSplit | null,
  limits: LaneLimits = LANE_LIMITS,
): BossSplit {
  const fresh = splitBoss(input, limits)
  const room = finite(input.room)
  const cardless = (split: BossSplit): boolean => split.heroCard === 0 || split.bossCard === 0
  if (held === null || cardless(held) || cardless(fresh)) return fresh
  if (Math.abs(held.opponents + held.own - room) > 0.5) return fresh
  const need = (board: BossBoard, card: number): number =>
    finite(board.head ?? 0) + laneHeight(board.width, board.counts, card, limits)
  const most = (boards: readonly BossBoard[], card: number): number =>
    Math.max(0, ...boards.map((board) => need(board, card)))
  const gap = input.allies.length === 0 ? 0 : finite(input.gap)
  // What each row lacks for its boards at a card; the boss's row may be capped and a row squeezed, so a held row serves
  // at a card while it lacks no more there than the fresh split's row does at the fresh card.
  const bossShort = (split: BossSplit, card: number): number =>
    finite(input.bossChrome) + most(input.boss, card) - split.opponents
  const heroShort = (split: BossSplit, card: number): number =>
    Math.max(
      input.allies.length === 0 ? 0 : finite(input.allyChrome) + most(input.allies, card) - split.band,
      finite(input.ownChrome) + need(input.own, card) - (split.own - split.band - gap),
    )
  const bossSlack = Math.max(0, bossShort(fresh, fresh.bossCard)) + 1
  const heroSlack = Math.max(0, heroShort(fresh, fresh.heroCard)) + 1
  const { at, rungs } = ladderOf(limits)
  const bossFits = (rung: number): boolean => bossShort(held, at(rung)) <= bossSlack
  const heroFits = (rung: number): boolean => heroShort(held, at(rung)) <= heroSlack
  if (!bossFits(0) || !heroFits(0)) return fresh
  const heroCard = at(highestRung(rungs, heroFits))
  const bossCard = Math.min(at(highestRung(rungs, bossFits)), Math.floor(heroCard * BOSS_SCALE))
  const more = (board: BossBoard): BossBoard => ({ ...board, counts: padded(board.counts) })
  const roomy = splitBoss(
    { ...input, boss: input.boss.map(more), allies: input.allies.map(more), own: more(input.own) },
    limits,
  )
  const better = (card: number, now: number, wider: number): boolean =>
    card * (1 + GROW_SHARE) < now && card * (1 + GROW_SHARE) < wider
  // A held boss under its fair share of the heroes' card gives way to a fresh split that draws it larger.
  const unfair = bossCard < heroCard * BOSS_FAIR_SHARE * (1 - GROW_SHARE / 2) && fresh.bossCard > bossCard
  const grows =
    unfair ||
    better(heroCard, fresh.heroCard, roomy.heroCard) ||
    better(bossCard, fresh.bossCard, roomy.bossCard)
  return grows ? fresh : { ...held, heroCard, bossCard }
}

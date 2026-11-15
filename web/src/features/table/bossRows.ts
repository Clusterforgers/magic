/** The boards of boss mode as `splitBoss` reads them: their widths in the table's frames and what each row spends besides lanes. */
import { allyShares, POD_MIN_PX, splitBoss } from './bossSplit'
import type { BossBoard, BossSplit } from './bossSplit'
import type { LaneCounts } from './laneFit'
import { BOSS_ABILITIES_PX, BOSS_SPAN, POD_COLUMN_PX, ZONE_GAP_PX } from './layout'
import type { Regions, SeatPlan } from './layout'

/** The padding of a battlefield's zone and the border and padding of a seat frame, one side, in px (`p-1`, `border-2 p-1`). */
const ZONE_PAD_PX = 4
const FRAME_PX = 6
/** The tile of an ally over its board (`h-14`) and the gap under it (`gap-1`). */
const ALLY_TILE_PX = 56 + 4
/** A minion's pod line over its board, and the gap under it. */
const MINION_HEAD_PX = 40 + ZONE_GAP_PX

/** What the boss-mode split reads of the table: the counts of every board and whether your hand has a card to play. */
export interface BossCounts {
  /** Your board. */
  own: LaneCounts
  /** The boss's board first, then each minion's. */
  theirs: readonly LaneCounts[]
  /** The other heroes' boards, in the party's order. */
  allies: readonly LaneCounts[]
  /** Your hand has a card, a commander or a top card to play. */
  ownReady: boolean
}

/** The split of the room of `regions` for the boss-mode boards, with the chrome of the boss's row (for the sheet's room). */
export function bossSplitOf(
  regions: Regions,
  seats: SeatPlan,
  counts: BossCounts,
  reserve: number,
  overhang: number,
): { split: BossSplit; shares: number[]; chrome: number } {
  const main = regions.main.width
  const inner = main - 2 * ZONE_GAP_PX
  const minions = Math.max(0, seats.boards.length - 1)
  const room = inner - minions * ZONE_GAP_PX
  const column = minions === 0 ? inner : (room * BOSS_SPAN) / (BOSS_SPAN + minions)
  const minion = minions === 0 ? 0 : room / (BOSS_SPAN + minions)
  const boss: BossBoard[] = counts.theirs.map((each, at) =>
    at === 0
      ? { width: column - 2 * FRAME_PX - POD_COLUMN_PX - ZONE_GAP_PX - 2 * ZONE_PAD_PX, counts: each }
      : { width: minion - 2 * FRAME_PX - 2 * ZONE_PAD_PX, counts: each, head: MINION_HEAD_PX },
  )
  const shares = allyShares(counts.allies)
  const allyGaps = Math.max(0, counts.allies.length - 1) * ZONE_GAP_PX
  const sum = shares.reduce((a, b) => a + b, 0)
  const allies: BossBoard[] = counts.allies.map((each, at) => ({
    width: ((inner - allyGaps) * (shares[at] ?? 0)) / Math.max(1, sum) - 2 * FRAME_PX - 2 * ZONE_PAD_PX,
    counts: each,
  }))
  const bossChrome =
    2 * ZONE_GAP_PX + BOSS_ABILITIES_PX + ZONE_GAP_PX + 2 * FRAME_PX + 2 * ZONE_PAD_PX + reserve
  const sheet = Math.ceil(Math.max(0, overhang) / 8) * 8
  const own: BossBoard = {
    width: main - 2 * ZONE_GAP_PX - POD_COLUMN_PX - ZONE_GAP_PX - 2 * ZONE_PAD_PX,
    counts: counts.own,
  }
  const split = splitBoss({
    room: regions.opponents + regions.own,
    boss,
    bossChrome,
    bossFloor: bossChrome - 2 * ZONE_PAD_PX + POD_MIN_PX + sheet - reserve,
    allies,
    allyChrome: 2 * FRAME_PX + ALLY_TILE_PX + 2 * ZONE_PAD_PX,
    own,
    ownChrome: 2 * ZONE_GAP_PX + 2 * ZONE_PAD_PX,
    ownReady: counts.ownReady,
    ownFloor: 2 * ZONE_GAP_PX + POD_MIN_PX,
    gap: ZONE_GAP_PX,
  })
  return { split, shares, chrome: bossChrome }
}

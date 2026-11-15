import { createContext } from 'react'
import type { PlayerId } from '@/protocol/generated'
import type { TableMode, Viewport } from './layout'

/** Where the table starts. `GameTable` takes no props, so a page that wants a start state provides it here. */
export interface TableOptions {
  /** The layout mode to open in; the rule of the seat count and the window's width when left out. */
  mode?: TableMode
  /** The opponent to open focused (focus mode); the one that follows the turn when left out. */
  focus?: PlayerId
  /** Decorate batches with animations (`features/table/fx`); on everywhere but under vitest when left out. */
  effects?: boolean
  /** Multiplies the duration of every animation, to look at one slowly; 1 when left out. */
  effectsSlow?: number
  /**
   * Draw equal permanents as one box with a "×N" chip (`group.ts`). On everywhere but under vitest when left out, so
   * that other tests see every card; a test of stacks turns it on.
   */
  stacks?: boolean
  /** Draw the timed turn cues (`features/table/turn`: banner, ticker, skipped marks); on everywhere but under vitest when left out. */
  cues?: boolean
  /** Lay the table out for this window size instead of the browser's, which a test has no use for. */
  viewport?: Viewport
  /** Start the table tour by itself in a browser that has not seen it and is not under automation; on everywhere but under vitest when left out. */
  tour?: boolean
  /** Play the sound cues of `features/sound` unless muted; on everywhere but under vitest when left out. */
  sounds?: boolean
  /** Fold the lands of a grid's opponents into their pods' lands chips (`LandFoldContext`); on everywhere but under vitest when left out. */
  foldLands?: boolean
  /** Show an attack at a player as a badge on the defender's pod instead of a line (`combat/AttackBadges`); on everywhere but under vitest when left out. */
  attackBadges?: boolean
}

/** Whether attacks at a player are badges when `TableOptions.attackBadges` is left out. */
export const ATTACK_BADGES_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Whether battlefields stack equal permanents when `TableOptions.stacks` is left out. */
export const STACKS_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Whether a grid's opponents fold their lands when `TableOptions.foldLands` is left out. */
export const FOLD_LANDS_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Whether the table tour starts by itself when `TableOptions.tour` is left out. */
export const TOUR_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Provide a `TableOptions` around `GameTable`; without one the table picks its own start. */
export const TableOptionsContext = createContext<TableOptions>({})

/**
 * The seats of the viewer's party where the layout keeps an attack at a player clear of the other boards, so that it is
 * drawn as a line to the defender's pod or tile as well as a badge on it, strong where a party seat attacks or is
 * attacked (boss mode: the boss's side above, the party below); null where badges stand in for the lines.
 */
export const AttackLinesContext = createContext<readonly PlayerId[] | null>(null)

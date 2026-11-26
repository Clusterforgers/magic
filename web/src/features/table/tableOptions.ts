import { createContext } from 'react'
import type { ReactNode } from 'react'
import type { PlayerId } from '@/protocol/generated'
import type { TableMode, Viewport } from './layout'
import type { SideTab } from './sideCounts'

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
  /** Give the boards of a grid and of a phone a lands chip that folds their lands (`LandFoldContext`, `landPrefs.ts`); on everywhere but under vitest when left out. */
  foldLands?: boolean
  /** Show a badge on the defender's pod or tile for each seat that attacks it, besides the lines (`combat/AttackBadges`); on everywhere but under vitest when left out. */
  attackBadges?: boolean
  /** The side tab to show once the table is up, after a decision has brought the Stack tab forward (the dev table's `?side=log`). */
  sideTab?: SideTab
}

/** Whether the pods carry attack badges when `TableOptions.attackBadges` is left out. */
export const ATTACK_BADGES_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Whether battlefields stack equal permanents when `TableOptions.stacks` is left out. */
export const STACKS_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Whether the boards carry a lands chip when `TableOptions.foldLands` is left out. */
export const FOLD_LANDS_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Whether the table tour starts by itself when `TableOptions.tour` is left out. */
export const TOUR_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Provide a `TableOptions` around `GameTable`; without one the table picks its own start. */
export const TableOptionsContext = createContext<TableOptions>({})

/**
 * Seats the layout counts as the viewer's party besides their teammates in the house rules (boss mode: the heroes
 * below), whose attacks and the attacks at them draw strong; null where it names none.
 */
export const AttackLinesContext = createContext<readonly PlayerId[] | null>(null)

/** What a spectator's table draws where the pass controls stand: a replay's own controls; "Watching" when null. */
export const SpectatorControlsContext = createContext<ReactNode>(null)

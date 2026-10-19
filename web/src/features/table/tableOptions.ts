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
  /** Lay the table out for this window size instead of the browser's, which a test has no use for. */
  viewport?: Viewport
}

/** Provide a `TableOptions` around `GameTable`; without one the table picks its own start. */
export const TableOptionsContext = createContext<TableOptions>({})

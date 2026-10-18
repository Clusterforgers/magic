import { createContext } from 'react'
import type { PlayerId } from '@/protocol/generated'
import type { TableMode } from './layout'

/** Where the table starts. `GameTable` takes no props, so a page that wants a start state provides it here. */
export interface TableOptions {
  /** The layout mode to open in; the default for the seat count when left out. */
  mode?: TableMode
  /** The opponent to open focused (focus mode); the one that follows the turn when left out. */
  focus?: PlayerId
}

/** Provide a `TableOptions` around `GameTable`; without one the table picks its own start. */
export const TableOptionsContext = createContext<TableOptions>({})

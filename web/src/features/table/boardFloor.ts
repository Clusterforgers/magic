/** The narrowest card a board draws before it scrolls, raised on a phone so its permanents stay readable. */
import { createContext } from 'react'

/** A phone's board floor in px: a compact face's name reads at this width, and the zone scrolls past it. */
export const PHONE_FLOOR_PX = 64

/** The board floor of the table, in px; null keeps the ladder's own floor (`LANE_LIMITS.minWidth`). */
export const BoardFloorContext = createContext<number | null>(null)

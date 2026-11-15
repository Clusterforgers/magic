/** What every board reads from the kind of window: a phone's card floor and whether the lands are laid out. */
import type { ReactNode } from 'react'
import { BoardFloorContext, PHONE_FLOOR_PX } from './boardFloor'
import { LandsShownContext, useLandsShown } from './landPrefs'

/** Provides the board floor and `LandsShownContext` (the viewer's "Show lands" choice, else by the window) to the table. */
export function BoardContexts({ phone, children }: { phone: boolean; children: ReactNode }) {
  const landsShown = useLandsShown(phone)
  return (
    <BoardFloorContext value={phone ? PHONE_FLOOR_PX : null}>
      <LandsShownContext value={landsShown}>{children}</LandsShownContext>
    </BoardFloorContext>
  )
}

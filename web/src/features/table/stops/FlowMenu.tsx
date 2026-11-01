import {
  DropdownMenuCheckboxItem,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu'
import { useGame, usePlay } from '@/features/game/gameContext'
import { resetDefaultStops, saveDefaultStops, useFlowPrefs } from '@/play/flowPrefs'
import { defaultStopsWords } from './stopModel'

/**
 * The pace settings at the foot of the Full Control menu: "End my turn by hand" (the server holds the seat at its own
 * end step until it ends the turn), and the pinned stops saved as the default of every new game, with a reset. Both are
 * kept in this browser and applied when a game is joined.
 */
export function FlowMenuItems() {
  const { session } = useGame()
  const prefs = useFlowPrefs()
  const stops = usePlay((state) => state.stops)
  const byHand = usePlay((state) => state.endTurnByHand)
  const online = usePlay((state) => state.status === 'open')
  const pinned = stops.filter((stop) => stop.keep)
  return (
    <>
      <DropdownMenuSeparator />
      <DropdownMenuCheckboxItem
        data-slot="end-turn-by-hand"
        checked={byHand}
        disabled={!online}
        onCheckedChange={(on) => session.setEndTurnByHand(on)}
      >
        End my turn by hand
      </DropdownMenuCheckboxItem>
      <DropdownMenuSeparator />
      <DropdownMenuGroup>
        <DropdownMenuLabel data-slot="default-stops">{defaultStopsWords(prefs.defaultStops.length)}</DropdownMenuLabel>
        <DropdownMenuItem
          data-slot="save-default-stops"
          disabled={pinned.length === 0}
          onClick={() => saveDefaultStops(pinned)}
        >
          Save pinned stops as default
        </DropdownMenuItem>
        <DropdownMenuItem
          data-slot="reset-default-stops"
          disabled={prefs.defaultStops.length === 0}
          onClick={resetDefaultStops}
        >
          Reset default stops
        </DropdownMenuItem>
      </DropdownMenuGroup>
    </>
  )
}

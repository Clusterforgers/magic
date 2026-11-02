import { X } from 'lucide-react'
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
 * end step until it ends the turn), the decisions the server answers for the seat outside Full Control (its triggers in
 * timestamp order, combat damage lethal in order), the abilities it always yields to (a click takes one back), and the
 * pinned stops saved as the default of every new game, with a reset. The end-turn setting and the default stops are kept
 * in this browser and applied when a game is joined.
 */
export function FlowMenuItems() {
  const { session } = useGame()
  const prefs = useFlowPrefs()
  const stops = usePlay((state) => state.stops)
  const byHand = usePlay((state) => state.endTurnByHand)
  const auto = usePlay((state) => state.autoChoices)
  const yields = usePlay((state) => state.autoYields)
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
      <DropdownMenuCheckboxItem
        data-slot="auto-order-triggers"
        checked={auto.order_triggers}
        disabled={!online}
        onCheckedChange={(on) => session.setAutoChoices({ ...auto, order_triggers: on })}
      >
        Order my triggers by timestamp
      </DropdownMenuCheckboxItem>
      <DropdownMenuCheckboxItem
        data-slot="auto-assign-damage"
        checked={auto.assign_damage}
        disabled={!online}
        onCheckedChange={(on) => session.setAutoChoices({ ...auto, assign_damage: on })}
      >
        Assign combat damage for me
      </DropdownMenuCheckboxItem>
      {yields.length === 0 ? null : (
        <>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuLabel data-slot="auto-yields">Always yielding to</DropdownMenuLabel>
            {yields.map((ability) => (
              <DropdownMenuItem
                key={`${ability.name}\n${ability.text}`}
                data-slot="clear-auto-yield"
                disabled={!online}
                title={ability.text === '' ? undefined : ability.text}
                aria-label={`Stop yielding to ${ability.name}${ability.text === '' ? '' : `: ${ability.text}`}`}
                onClick={() => session.clearAutoYield(ability)}
              >
                <span className="min-w-0 flex-1 truncate">{ability.name}</span>
                <X aria-hidden="true" className="text-muted-foreground" />
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        </>
      )}
      <DropdownMenuSeparator />
      <DropdownMenuGroup>
        <DropdownMenuLabel data-slot="default-stops">
          {defaultStopsWords(prefs.defaultStops.length)}
        </DropdownMenuLabel>
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

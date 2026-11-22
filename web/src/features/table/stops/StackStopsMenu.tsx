import { Layers } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { StackStops } from '@/protocol/generated'
import { canSetStops } from '../selectors'
import { STACK_STOP_LEVELS, stackStopsOf, withStackStops } from '@/play/stackStops'
import { STACK_STOPS_WORDS, stackStopsName } from './stackStops'

/**
 * "Stack stops" beside "Auto-skip triggers": one button whose menu picks which spells and abilities on the stack the
 * seat is asked at (`AutoChoices.stack_stops`, saved with the account like the other auto choices): all of them, only
 * opponents', or only what targets the seat or its side. The button is filled while a level other than the first is
 * held and shows what the server holds, so it changes when the server says so. Full Control asks at everything while
 * it is on.
 */
export function StackStopsMenu() {
  const { session } = useGame()
  const { shown, auto, online, paused } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      auto: state.autoChoices,
      online: state.status === 'open',
      paused: state.fullControl !== 'Off',
    })),
  )
  if (!shown) return null
  const level = stackStopsOf(auto)
  const name = stackStopsName(level)
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        data-slot="stack-stops"
        data-level={level}
        aria-label={name}
        title={paused ? `${name}. Off while Full Control is on.` : name}
        disabled={!online}
        render={<Button size="icon-sm" variant={level === 'All' ? 'outline' : 'default'} />}
      >
        <Layers aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-72 max-w-[calc(100vw-1rem)]">
        <DropdownMenuGroup>
          <DropdownMenuLabel>
            Ask me on the stack at
            {paused ? <span className="font-normal"> (off in Full Control)</span> : null}
          </DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={level}
            onValueChange={(next) => session.setAutoChoices(withStackStops(auto, next as StackStops))}
          >
            {STACK_STOP_LEVELS.map((choice) => (
              <DropdownMenuRadioItem
                key={choice}
                value={choice}
                data-level={choice}
                closeOnClick
                aria-description={STACK_STOPS_WORDS[choice].description}
                className="items-start"
              >
                <span className="grid gap-0.5">
                  <span>{STACK_STOPS_WORDS[choice].label}</span>
                  <span aria-hidden="true" className="text-xs font-normal text-muted-foreground">
                    {STACK_STOPS_WORDS[choice].description}
                  </span>
                </span>
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

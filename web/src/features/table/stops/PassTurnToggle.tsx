import { cn } from '@/lib/utils'
import { useShallow } from 'zustand/react/shallow'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { useGame, usePlay } from '@/features/game/gameContext'
import { YIELD_WORDS } from '../prompt/passWords'
import type { YieldUntil } from '@/protocol/generated'
import { canSetStops } from '../selectors'

/** The yield this switch holds. */
export const PASS_TURN = 'MyTurnThrough' satisfies YieldUntil

const WORDS = YIELD_WORDS[PASS_TURN]

/**
 * "Pass until my turn" beside "Auto-skip triggers": a switch that asks the server to pass for the seat until its own
 * next turn (`Yield { MyTurnThrough }`), asking only the decisions with a real choice. It shows the yield the server
 * holds, never a guess: on with the server's `Yielding`, off when the seat's turn begins, it acts by itself or it
 * presses again. Full Control asks every window, so the switch is off and unavailable while it is on.
 */
export function PassTurnToggle() {
  const { session } = useGame()
  const { shown, on, online, paused } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      on: state.yielding === PASS_TURN,
      online: state.status === 'open',
      paused: state.fullControl !== 'Off',
    })),
  )
  if (!shown) return null
  const description = paused ? `${WORDS.description}. Off while Full Control is on.` : WORDS.description
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <label
            data-slot="pass-turn"
            data-on={on}
            className="flex h-8 items-center gap-1.5 px-1 text-xs font-medium"
          />
        }
      >
        <Switch
          data-slot="pass-turn-switch"
          checked={on}
          disabled={!online || paused}
          aria-label={WORDS.action}
          aria-description={description}
          onCheckedChange={(checked) => {
            if (checked) session.yieldUntil(PASS_TURN)
            else session.cancelYield()
          }}
        />
        <span aria-hidden="true" className={cn(paused && 'opacity-60')}>
          {WORDS.action}
        </span>
      </TooltipTrigger>
      <TooltipContent side="top">{description}</TooltipContent>
    </Tooltip>
  )
}

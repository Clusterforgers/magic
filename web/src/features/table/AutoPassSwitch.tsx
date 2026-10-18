import { useShallow } from 'zustand/react/shallow'
import { Switch } from '@/components/ui/switch'
import { useGame, usePlay } from '@/features/game/gameContext'
import { canSetAutoPass } from './selectors'

/** The words on the switch. */
export const AUTO_PASS_LABEL = "Skip turns when I can't act"

/**
 * "Skip turns when I can't act": while it is on, the server passes priority for this seat whenever there is nothing
 * to play (no land, no spell it can pay for), so the game never waits on a "nothing to do" prompt. The switch shows
 * what the server holds for the seat and cannot be flipped while the connection is down, since the server would
 * not hear it. It is drawn only for a seat that is still in a running game.
 */
export function AutoPassSwitch() {
  const { session } = useGame()
  const { shown, on, online } = usePlay(
    useShallow((state) => ({
      shown: canSetAutoPass(state),
      on: state.autoPass,
      online: state.status === 'open',
    })),
  )
  if (!shown) return null
  return (
    <label className="inline-flex items-center gap-2 text-sm select-none">
      <Switch checked={on} disabled={!online} onCheckedChange={(next) => session.setAutoPass(next)} />
      {AUTO_PASS_LABEL}
    </label>
  )
}

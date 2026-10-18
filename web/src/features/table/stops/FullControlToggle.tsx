import { ChevronDown, Gamepad2 } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { FullControlMode } from '@/protocol/generated'
import { canSetStops } from '../selectors'
import {
  FULL_CONTROL_DESCRIPTION,
  FULL_CONTROL_KEYS,
  FULL_CONTROL_STATE,
  fullControlName,
  toggledMode,
} from './fullControlWords'
import { useFullControlKeys } from './useFullControlKeys'

/** The choices in the menu, with the keys that ask for them. */
const CHOICES: { mode: FullControlMode; label: string; keys?: string }[] = [
  { mode: 'Off', label: 'Off' },
  { mode: 'NextWindow', label: 'Next window only', keys: FULL_CONTROL_KEYS.NextWindow },
  { mode: 'Held', label: 'Held until I turn it off', keys: FULL_CONTROL_KEYS.Held },
]

/**
 * Full Control: the button asks for it held (and turns it off again), its menu for any of the three modes, and Control
 * or Control with Shift do the same from the keyboard. It shows what the server holds, in words, so it changes when
 * the server says so and not when it is pressed; the server also turns it off by itself after the next window. It is
 * drawn only for a seat that is still in a running game, and cannot be used while the connection is down.
 */
export function FullControlToggle() {
  const { session } = useGame()
  const { shown, mode, online } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      mode: state.fullControl,
      online: state.status === 'open',
    })),
  )
  useFullControlKeys(shown)
  if (!shown) return null

  const on = mode !== 'Off'
  const ask = (next: FullControlMode): void => {
    if (online) session.setFullControl(next)
  }
  return (
    <div
      role="group"
      aria-label="Full Control"
      data-slot="full-control"
      data-mode={mode}
      className="inline-flex items-center gap-0.5"
    >
      <Button
        data-slot="full-control-toggle"
        size="sm"
        variant={on ? 'default' : 'outline'}
        aria-pressed={on}
        aria-label={fullControlName(mode)}
        aria-description={FULL_CONTROL_DESCRIPTION[mode]}
        aria-keyshortcuts="Control Control+Shift"
        aria-disabled={online ? undefined : true}
        title={`${FULL_CONTROL_DESCRIPTION[mode]} ${FULL_CONTROL_KEYS.NextWindow} asks for the next window, ${FULL_CONTROL_KEYS.Held} holds it.`}
        className="aria-disabled:pointer-events-none aria-disabled:opacity-50"
        onClick={() => ask(toggledMode(mode))}
      >
        <Gamepad2 aria-hidden="true" />
        <span>Full Control</span>
        <span data-slot="full-control-state" className="font-normal opacity-80">
          {FULL_CONTROL_STATE[mode]}
        </span>
      </Button>
      <DropdownMenu>
        <DropdownMenuTrigger
          data-slot="full-control-menu"
          aria-label="Full Control options"
          disabled={!online}
          render={<Button size="icon-sm" variant="outline" />}
        >
          <ChevronDown aria-hidden="true" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuRadioGroup value={mode} onValueChange={(next) => ask(next as FullControlMode)}>
            {CHOICES.map(({ mode: choice, label, keys }) => (
              <DropdownMenuRadioItem
                key={choice}
                value={choice}
                data-mode={choice}
                closeOnClick
                aria-keyshortcuts={keys === undefined ? undefined : keys.replace(/Ctrl/g, 'Control')}
              >
                {label}
                {keys === undefined ? null : (
                  <kbd className="ml-auto pl-4 text-xs font-normal text-muted-foreground">{keys}</kbd>
                )}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}

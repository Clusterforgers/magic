import { CheckIcon, CircleAlertIcon, Globe, Lock } from 'lucide-react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { VISIBILITY_CHOICES, VISIBILITY_WORDS } from '@/lib/visibility'
import type { Visibility } from '@/protocol/generated'
import type { VisibilityControl } from './useVisibility'

const ICONS = { public: Globe, invite: Lock } as const

interface VisibilityToggleProps {
  /** What the toggle shows now (the value being asked for while its request is out). */
  shown: Visibility
  control: VisibilityControl
  /** Some request of the lobby is out, so nothing else can be sent. */
  busy: boolean
}

/**
 * The host's way to list the game or make it invite only: two toggle buttons, so both choices read in words at all
 * times. The pressed one has a check as well as its border. The one asked for shows a spinner, the other waits, and a
 * polite line says what is happening and what came of it; a refusal puts the old value back and says why.
 */
export function VisibilityToggle({ shown, control, busy }: VisibilityToggleProps) {
  const { pending, note, problem, choose } = control
  return (
    <div
      data-slot="lobby-visibility"
      data-visibility={shown}
      data-pending={pending ?? undefined}
      className="mt-3 grid gap-2"
    >
      <div role="group" aria-labelledby="visibility-title" className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span id="visibility-title" className="text-sm text-muted-foreground">
          Who can find this game
        </span>
        <div className="flex flex-wrap gap-1 rounded-lg bg-muted p-1">
          {VISIBILITY_CHOICES.map((choice) => {
            const selected = shown === choice
            const Icon = selected ? CheckIcon : ICONS[choice]
            return (
              <Button
                key={choice}
                type="button"
                size="sm"
                variant={selected ? 'outline' : 'ghost'}
                aria-pressed={selected}
                loading={pending === choice}
                disabled={busy && pending !== choice}
                className={selected ? 'border-primary' : undefined}
                onClick={() => choose(choice)}
              >
                {pending === choice ? null : <Icon aria-hidden="true" />}
                {VISIBILITY_WORDS[choice].status}
              </Button>
            )
          })}
        </div>
      </div>
      <p
        data-slot="visibility-note"
        aria-live="polite"
        aria-atomic="true"
        className="text-sm text-muted-foreground empty:hidden"
      >
        {note}
      </p>
      {problem === null ? null : (
        <Alert variant="destructive" data-slot="visibility-problem">
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">{problem}</AlertDescription>
        </Alert>
      )}
    </div>
  )
}

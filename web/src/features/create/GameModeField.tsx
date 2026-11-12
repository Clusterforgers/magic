import { MODE_WORDS } from '@/lib/teams'
import type { GameMode } from '@/lib/teams'

const MODES: readonly GameMode[] = ['free_for_all', 'two_headed_giant', 'teams']

interface GameModeFieldProps {
  value: GameMode
  /** Why the seats do not fit the mode, or null. */
  problem: string | null
  onChange(mode: GameMode): void
}

/** How the table plays: Free-for-all, Two-Headed Giant (4 seats) or Teams (an even count from 4). */
export function GameModeField({ value, problem, onChange }: GameModeFieldProps) {
  return (
    <fieldset
      className="grid gap-2"
      data-slot="mode-field"
      aria-describedby={problem === null ? undefined : 'mode-problem'}
    >
      <legend className="mb-1 text-lg font-semibold">Game mode</legend>
      <div className="grid gap-2 sm:grid-cols-3 md:grid-cols-1 lg:grid-cols-3">
        {MODES.map((mode) => (
          <label
            key={mode}
            className="flex cursor-pointer items-start gap-3 rounded-lg border border-border-strong px-3 py-2 has-checked:border-primary has-checked:bg-muted has-focus-visible:ring-3 has-focus-visible:ring-ring"
          >
            <input
              type="radio"
              name="mode"
              value={mode}
              checked={value === mode}
              aria-invalid={value === mode && problem !== null}
              className="mt-1 accent-primary"
              onChange={() => onChange(mode)}
            />
            <span className="grid gap-1">
              <span className="text-base font-medium">{MODE_WORDS[mode].name}</span>
              <span className="text-sm text-muted-foreground">{MODE_WORDS[mode].words}</span>
            </span>
          </label>
        ))}
      </div>
      {problem === null ? null : (
        <p id="mode-problem" className="text-sm text-destructive">
          {problem}
        </p>
      )}
    </fieldset>
  )
}

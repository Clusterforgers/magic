import { useId } from 'react'
import { VISIBILITY_CHOICES, VISIBILITY_WORDS } from '@/lib/visibility'
import type { Visibility } from '@/protocol/generated'

interface VisibilityFieldProps {
  value: Visibility
  onChange(value: Visibility): void
}

/** Who may find the game: each choice with what it means beside it, so nobody picks one without knowing the cost. */
export function VisibilityField({ value, onChange }: VisibilityFieldProps) {
  const id = useId()
  return (
    <fieldset className="grid gap-2">
      <legend className="mb-1 text-lg font-semibold">Visibility</legend>
      {VISIBILITY_CHOICES.map((choice) => {
        const words = VISIBILITY_WORDS[choice]
        const note = `${id}-${choice}`
        return (
          <div
            key={choice}
            className="relative flex flex-wrap items-baseline gap-x-3 gap-y-1 rounded-lg border border-border-strong px-3 py-2 has-checked:border-primary has-checked:bg-muted pointer-coarse:py-3"
          >
            {/* The label's pseudo-element covers the card, so a press anywhere on it chooses. */}
            <label className="flex items-center gap-2 text-sm font-medium after:absolute after:inset-0 after:content-['']">
              <input
                type="radio"
                name="visibility"
                value={choice}
                checked={value === choice}
                aria-describedby={note}
                className="accent-primary"
                onChange={() => onChange(choice)}
              />
              {words.choice}
            </label>
            <p id={note} className="text-sm text-muted-foreground">
              {words.consequence}
            </p>
          </div>
        )
      })}
    </fieldset>
  )
}

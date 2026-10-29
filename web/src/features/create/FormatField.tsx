import { formatRuleWords } from '@/lib/houseRules'
import type { GameFormat } from '@/lib/houseRules'

const FORMATS: readonly { value: GameFormat; name: string; words: string }[] = [
  {
    value: 'commander',
    name: 'Commander',
    words: '2 to 6 players, 40 life, a commander each',
  },
  {
    value: 'normal',
    name: 'Normal',
    words: '2 to 12 players, 20 life, 60-card decks',
  },
]

interface FormatFieldProps {
  value: GameFormat
  onChange(format: GameFormat): void
}

/** The format of the game: Commander, the default, or Normal. The seats, the decks and the house rules follow it. */
export function FormatField({ value, onChange }: FormatFieldProps) {
  return (
    <fieldset className="grid gap-2" data-slot="format-field">
      <legend className="mb-1 text-lg font-semibold">Format</legend>
      <div className="grid gap-2 sm:grid-cols-2">
        {FORMATS.map((format) => (
          <label
            key={format.value}
            className="flex cursor-pointer items-start gap-3 rounded-lg border border-border-strong px-3 py-2 has-checked:border-primary has-checked:bg-muted has-focus-visible:ring-3 has-focus-visible:ring-ring"
          >
            <input
              type="radio"
              name="format"
              value={format.value}
              checked={value === format.value}
              className="mt-1 accent-primary"
              onChange={() => onChange(format.value)}
            />
            <span className="grid gap-1">
              <span className="text-base font-medium">{format.name}</span>
              <span className="text-sm text-muted-foreground">{format.words}</span>
            </span>
          </label>
        ))}
      </div>
      {value === 'commander' ? (
        <p data-slot="format-rules" className="text-sm text-muted-foreground">
          <span className="font-medium text-foreground">Commander rules: </span>
          {formatRuleWords('commander').join(' · ')}. Change any of them under House rules.
        </p>
      ) : null}
    </fieldset>
  )
}

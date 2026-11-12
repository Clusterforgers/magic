import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import type { BotLevel } from '@/net/api'

interface BotsFieldProps {
  /** The bots chosen now. */
  value: number
  /** The most there can be: every seat but the creator's. */
  most: number
  /** How well the bots play. */
  level: BotLevel
  /** Called with the count chosen. */
  onChange(count: number): void
  /** Called with the level chosen. */
  onLevel(level: BotLevel): void
}

/** The levels a bot plays at, with what each means. */
const LEVELS: readonly { level: BotLevel; label: string }[] = [
  { level: 'easy', label: 'Easy — random plays' },
  { level: 'normal', label: 'Normal — plays sensibly' },
  { level: 'hard', label: 'Hard — thinks ahead' },
]

/** How many of the other seats bots play (none, or from one up to every seat but the creator's) and how well. */
export function BotsField({ value, most, level, onChange, onLevel }: BotsFieldProps) {
  const counts = Array.from({ length: most + 1 }, (_, count) => count)
  const shown = Math.min(value, most)
  return (
    <div className="grid gap-2">
      <Label htmlFor="bot-count">Bots</Label>
      <NativeSelect
        id="bot-count"
        data-slot="bot-count"
        value={String(shown)}
        aria-describedby="bot-count-help"
        onChange={(event) => onChange(Number(event.target.value))}
      >
        {counts.map((count) => (
          <NativeSelectOption key={count} value={String(count)}>
            {count === 0 ? 'No bots' : count === most ? `${count} — every other seat` : String(count)}
          </NativeSelectOption>
        ))}
      </NativeSelect>
      {shown > 0 ? (
        <>
          <Label htmlFor="bot-level">Difficulty</Label>
          <NativeSelect
            id="bot-level"
            data-slot="bot-level"
            value={level}
            onChange={(event) => onLevel(event.target.value as BotLevel)}
          >
            {LEVELS.map((option) => (
              <NativeSelectOption key={option.level} value={option.level}>
                {option.label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </>
      ) : null}
      <p id="bot-count-help" className="text-sm text-muted-foreground">
        Bots take the seats after yours with a deck of their own and play at a pace you can follow.
      </p>
    </div>
  )
}

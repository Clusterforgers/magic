import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'

interface BotsFieldProps {
  /** The bots chosen now. */
  value: number
  /** The most there can be: every seat but the creator's. */
  most: number
  /** Called with the count chosen. */
  onChange(count: number): void
}

/** How many of the other seats bots play: none, or from one up to every seat but the creator's. */
export function BotsField({ value, most, onChange }: BotsFieldProps) {
  const counts = Array.from({ length: most + 1 }, (_, count) => count)
  return (
    <div className="grid gap-2">
      <Label htmlFor="bot-count">Bots</Label>
      <NativeSelect
        id="bot-count"
        data-slot="bot-count"
        value={String(Math.min(value, most))}
        aria-describedby="bot-count-help"
        onChange={(event) => onChange(Number(event.target.value))}
      >
        {counts.map((count) => (
          <NativeSelectOption key={count} value={String(count)}>
            {count === 0 ? 'No bots' : count === most ? `${count} — every other seat` : String(count)}
          </NativeSelectOption>
        ))}
      </NativeSelect>
      <p id="bot-count-help" className="text-sm text-muted-foreground">
        Bots take the seats after yours with a deck of their own and play at a pace you can follow.
      </p>
    </div>
  )
}

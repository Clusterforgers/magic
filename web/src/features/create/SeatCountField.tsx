import { useState } from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { MIN_SEATS } from '@/lib/limits'
import { seatsTyped } from './form'

interface SeatCountFieldProps {
  /** The seats the form has. */
  count: number
  /** A count the person typed that makes a game: a whole number from `MIN_SEATS` up. */
  onCount(count: number): void
}

/** How many seats the game has: a number field with no most, so a table of any size can be asked for. */
export function SeatCountField({ count, onCount }: SeatCountFieldProps) {
  // The text while it is being typed; "1" on the way to "12" is not a count, and the field must let it be.
  const [typed, setTyped] = useState<string | null>(null)
  const problem = typed !== null && seatsTyped(typed) === null
  return (
    <div className="grid gap-2">
      <Label htmlFor="seat-count">Seats</Label>
      <Input
        id="seat-count"
        type="number"
        inputMode="numeric"
        min={MIN_SEATS}
        step={1}
        value={typed ?? count}
        aria-invalid={problem}
        aria-describedby="seat-count-help"
        onChange={(event) => {
          setTyped(event.target.value)
          const typedCount = seatsTyped(event.target.value)
          if (typedCount !== null) onCount(typedCount)
        }}
        onBlur={() => setTyped(null)}
      />
      <p id="seat-count-help" className="text-sm text-muted-foreground">
        {problem
          ? `A game needs at least ${MIN_SEATS} seats.`
          : 'The game starts when every seat is taken and ready.'}
      </p>
    </div>
  )
}

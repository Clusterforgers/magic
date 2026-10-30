import { Button } from '@/components/ui/button'
import { AmountField } from './AmountField'
import { ROLL_PICKS, SIDES } from './roll'
import type { RollPick } from './roll'

interface Props {
  pick: RollPick
  sides: number
  onPick: (pick: RollPick) => void
  onSides: (sides: number) => void
}

/** The choices of the Roll row: a coin, a d6, a d20 or a die of any size, with the size field for the last. */
export function RollChoice({ pick, sides, onPick, onSides }: Props) {
  return (
    <div className="grid gap-2">
      <div role="group" aria-label="What to roll" className="flex flex-wrap items-center gap-1">
        {ROLL_PICKS.map((choice) => (
          <Button
            key={choice.pick}
            variant={choice.pick === pick ? 'default' : 'outline'}
            size="sm"
            aria-pressed={choice.pick === pick}
            data-roll={choice.pick}
            className="min-w-10 px-2 tabular-nums pointer-coarse:h-11 pointer-coarse:min-w-11"
            onClick={() => onPick(choice.pick)}
          >
            {choice.label}
          </Button>
        ))}
      </div>
      {pick === 'custom' ? <AmountField spec={SIDES} value={sides} onChange={onSides} label="Faces" /> : null}
    </div>
  )
}

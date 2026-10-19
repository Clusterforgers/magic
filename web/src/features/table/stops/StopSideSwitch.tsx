import { Button } from '@/components/ui/button'
import type { StopSide } from '@/protocol/generated'
import { SIDE_WORD, sideName, sideTitle } from './stopModel'

const SIDES: readonly StopSide[] = ['Own', 'Opponent']

interface StopSideSwitchProps {
  /** The side whose steps are drawn. */
  shown: StopSide
  /** The side whose turn it is, or null outside the turns. */
  turn: StopSide | null
  /** How many stops the server holds on each side. */
  counts: Record<StopSide, number>
  onPick: (side: StopSide) => void
}

/** "Mine" or "Theirs": which side's stops the one row of steps shows; the side out of sight shows how many stops it holds. */
export function StopSideSwitch({ shown, turn, counts, onPick }: StopSideSwitchProps) {
  return (
    <div
      role="group"
      aria-label="Stops for"
      data-slot="stop-side"
      className="inline-flex shrink-0 gap-0.5 rounded-lg bg-muted p-0.5"
    >
      {SIDES.map((side) => (
        <Button
          key={side}
          data-side={side}
          data-turn={turn === side || undefined}
          size="sm"
          variant={shown === side ? 'default' : 'ghost'}
          className="gap-1 px-2 text-xs"
          aria-pressed={shown === side}
          aria-label={sideName(side, counts[side])}
          title={sideTitle(side)}
          onClick={() => onPick(side)}
        >
          {SIDE_WORD[side]}
          {/* The row on show lights its own stops; the count is for the side that is out of sight. */}
          {shown !== side && counts[side] > 0 ? (
            <span aria-hidden="true" className="rounded bg-foreground/15 px-1 text-[0.65rem] leading-4">
              {counts[side]}
            </span>
          ) : null}
        </Button>
      ))}
    </div>
  )
}

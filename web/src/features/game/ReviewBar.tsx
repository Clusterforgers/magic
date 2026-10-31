import { Link } from '@tanstack/react-router'
import { useShallow } from 'zustand/react/shallow'
import { Button, buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useGame, usePlay } from './gameContext'
import { describeResult } from './outcome'
import { seatName } from './useGameInfo'

/**
 * The strip over the final table while a finished game is looked at: the result in one line, a button that brings the
 * result screen back, and the way out to Play, so leaving stays one click. The table beneath takes no action (the
 * session is closed), and its Log and Stack tabs read as in play.
 */
export function ReviewBar({ onResult }: { onResult: () => void }) {
  const { lookup } = useGame()
  const { result, seat, view } = usePlay(
    useShallow((state) => ({ result: state.result, seat: state.seat, view: state.view })),
  )
  if (result === null) return null
  const { headline } = describeResult(result, seat, (who) => seatName(lookup, who), view?.players ?? [])
  return (
    <section
      aria-label="Game over, looking back"
      data-slot="review-bar"
      className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border bg-popover py-1 pr-1 pl-3 text-sm text-popover-foreground shadow-e2"
    >
      <span className="font-medium">{headline}</span>
      <span className="text-muted-foreground">Final table, read only</span>
      <span className="ml-auto flex gap-1">
        <Button size="sm" data-slot="review-close" onClick={onResult}>
          Show result
        </Button>
        <Link to="/" className={cn(buttonVariants({ size: 'sm', variant: 'outline' }))}>
          Back to Play
        </Link>
      </span>
    </section>
  )
}

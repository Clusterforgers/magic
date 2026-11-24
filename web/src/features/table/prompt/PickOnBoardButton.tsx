/** The switch of Pick on board (`pickOnBoard.ts`). */
import { EyeOff, LayoutGrid } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { KeyHint } from './KeyHint'
import type { PickOnBoard } from './pickOnBoard'

/**
 * The switch: "Pick on board" in the overlay, "Show choices" with the answers while the overlay is hidden; a phone's
 * bar draws the second as its icon alone (`promptBand.css`), so the answers keep one row.
 */
export function PickOnBoardButton({ pick }: { pick: PickOnBoard }) {
  const words = pick.hidden ? 'Show choices' : 'Pick on board'
  return (
    <Button
      variant="outline"
      size="sm"
      data-slot="pick-on-board"
      aria-label={words}
      aria-pressed={pick.hidden}
      aria-keyshortcuts="V"
      title={`${words} (V)`}
      onClick={pick.flip}
      className="shrink-0 pointer-coarse:h-11"
    >
      {pick.hidden ? <LayoutGrid aria-hidden="true" /> : <EyeOff aria-hidden="true" />}
      <span data-slot="pick-words">{words}</span>
      <KeyHint>V</KeyHint>
    </Button>
  )
}

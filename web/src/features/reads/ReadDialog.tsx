import { useContext } from 'react'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { ReadsApiContext } from './apiContext'
import { ReadCardView } from './ReadCardView'

interface ReadDialogProps {
  /** The card to read; the dialog is open while one is given. */
  oracleId: string | null
  /** The card's name, for the title before its rules arrive. */
  name: string
  onClose: () => void
}

/**
 * One card's read sheet and verdict in a dialog, opened from the table's card preview: a player who sees a card misbehave
 * reads it beside the game's words where it happens (docs/web.md, Reading cards).
 */
export function ReadDialog({ oracleId, name, onClose }: ReadDialogProps) {
  const api = useContext(ReadsApiContext)
  return (
    <Dialog
      open={oracleId !== null}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DialogContent data-slot="read-dialog">
        <DialogTitle>Read the rules of {name}</DialogTitle>
        <DialogDescription>
          Each printed clause beside what the game does with it. Your verdict counts from the next game.
        </DialogDescription>
        {oracleId === null ? null : (
          <ReadCardView
            api={api}
            oracleId={oracleId}
            named={false}
            verdictClassName="sticky -bottom-6 -mx-6 -mb-6 border-t bg-popover px-6 py-4"
          />
        )}
      </DialogContent>
    </Dialog>
  )
}

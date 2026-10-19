import { useState } from 'react'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { DeckError } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { DeckListItem } from '@/protocol/generated'
import { describeDeckError } from './errors'

interface DeleteDeckDialogProps {
  api: DecksApi
  deck: DeckListItem
  /** The deck is gone from the server. */
  onDeleted: (deck: DeckListItem) => void
  /** The dialog has closed, by either button, Escape or a successful delete; the caller unmounts it. */
  onClosed: () => void
}

/**
 * "Delete <name>?", asked before anything is sent: the safe choice is first and takes the focus. A refused delete keeps
 * the question up with the reason; a deck that is already gone counts as deleted.
 */
export function DeleteDeckDialog({ api, deck, onDeleted, onClosed }: DeleteDeckDialogProps) {
  const [open, setOpen] = useState(true)
  const [pending, setPending] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  async function confirm(): Promise<void> {
    setPending(true)
    setProblem(null)
    try {
      await api.remove(deck.id)
      onDeleted(deck)
      setOpen(false)
    } catch (error) {
      if (error instanceof DeckError && error.code === 'not_found') {
        onDeleted(deck)
        setOpen(false)
      } else {
        setProblem(describeDeckError(error))
      }
    } finally {
      setPending(false)
    }
  }

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        if (!pending) setOpen(next)
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed()
      }}
    >
      <AlertDialogContent>
        <AlertDialogTitle>{`Delete “${deck.name}”?`}</AlertDialogTitle>
        <AlertDialogDescription>
          The deck is removed from your account and cannot be brought back. Export it first if you may want it
          again.
        </AlertDialogDescription>
        {problem === null ? null : (
          <p role="alert" className="rounded-md bg-destructive/10 px-2 py-1 text-sm text-destructive">
            {problem}
          </p>
        )}
        <AlertDialogFooter>
          <Button type="button" variant="outline" onClick={() => setOpen(false)}>
            Keep deck
          </Button>
          <Button type="button" variant="destructive" loading={pending} onClick={() => void confirm()}>
            Delete deck
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

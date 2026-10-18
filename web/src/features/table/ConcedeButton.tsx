import { useState } from 'react'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import { canConcede, seatCountOf } from './selectors'

/**
 * "Concede", which asks "Concede the game?" first and sends the concession only on a second, explicit click. It is
 * not drawn for a seat that is already out or once the game is over. A refused send (the socket is not open) keeps
 * the question up with a note, so the click is never silently lost.
 */
export function ConcedeButton() {
  const { session } = useGame()
  const allowed = usePlay(canConcede)
  const seats = usePlay(seatCountOf)
  const [open, setOpen] = useState(false)
  const [refused, setRefused] = useState(false)
  if (!allowed) return null

  const change = (next: boolean): void => {
    setOpen(next)
    setRefused(false)
  }
  const confirm = (): void => {
    if (session.concede()) change(false)
    else setRefused(true)
  }
  return (
    <>
      <Button
        size="sm"
        variant="outline"
        className="text-destructive"
        aria-haspopup="dialog"
        onClick={() => change(true)}
      >
        Concede
      </Button>
      <AlertDialog open={open} onOpenChange={change}>
        <AlertDialogContent>
          <AlertDialogTitle>Concede the game?</AlertDialogTitle>
          <AlertDialogDescription>
            You lose the game at once and cannot take it back.
            {seats > 2 ? ' The other players play on.' : ' Your opponent wins.'}
          </AlertDialogDescription>
          {refused ? (
            <p role="alert" className="rounded-md bg-destructive/10 px-2 py-1 text-sm text-destructive">
              Could not reach the game. Check the connection and try again.
            </p>
          ) : null}
          <AlertDialogFooter>
            <Button variant="outline" onClick={() => change(false)}>
              Keep playing
            </Button>
            <Button variant="destructive" onClick={confirm}>
              Concede
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

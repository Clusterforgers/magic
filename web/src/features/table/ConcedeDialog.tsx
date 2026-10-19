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
import { seatCountOf } from './selectors'

interface ConcedeDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/**
 * The question "Concede the game?", which sends the concession only on a second, explicit click. A refused send (the
 * socket is not open) keeps the question up with a note, so the click is never silently lost. Whoever opens it (an
 * item of the table menu) owns `open`.
 */
export function ConcedeDialog({ open, onOpenChange }: ConcedeDialogProps) {
  const { session } = useGame()
  const seats = usePlay(seatCountOf)
  const [refused, setRefused] = useState(false)

  const change = (next: boolean): void => {
    onOpenChange(next)
    setRefused(false)
  }
  const confirm = (): void => {
    if (session.concede()) change(false)
    else setRefused(true)
  }
  return (
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
  )
}

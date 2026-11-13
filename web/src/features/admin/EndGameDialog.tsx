import { CircleAlertIcon } from 'lucide-react'
import { useId, useState } from 'react'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { AdminGamesApi } from '@/net/adminGames'

/** Most characters in the reason, as the server allows. */
export const MAX_REASON_CHARS = 200

interface EndGameDialogProps {
  api: AdminGamesApi
  /** The game to end. */
  game: string
  /** How the list names it, such as "Ann vs Bo". */
  title: string
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The game is gone; the list drops its row. */
  onEnded: () => void
}

/**
 * The question before an admin ends a game. It says what happens to the players and takes an optional reason they
 * will read; it starts in the reason field and cannot close while the request runs.
 */
export function EndGameDialog({ api, game, title, open, onOpenChange, onEnded }: EndGameDialogProps) {
  const [reason, setReason] = useState('')
  const [pending, setPending] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)
  const reasonId = useId()

  async function end(): Promise<void> {
    if (pending) return
    setPending(true)
    setProblem(null)
    try {
      await api.endGame(game, reason)
      setReason('')
      onOpenChange(false)
      onEnded()
    } catch (error) {
      setProblem(error instanceof Error ? error.message : 'The game could not be ended.')
    } finally {
      setPending(false)
    }
  }

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        if (pending) return
        onOpenChange(next)
        if (!next) setProblem(null)
      }}
    >
      <AlertDialogContent>
        <AlertDialogTitle>{`End ${title}?`}</AlertDialogTitle>
        <AlertDialogDescription>
          The game stops at once and has no winner. Its players and spectators are told it was ended by an
          admin, with your reason, and nobody can open it again.
        </AlertDialogDescription>
        <div className="grid gap-2">
          <Label htmlFor={reasonId}>Reason (optional)</Label>
          <Input
            id={reasonId}
            value={reason}
            maxLength={MAX_REASON_CHARS}
            placeholder="For example: the server restarts tonight"
            onChange={(event) => setReason(event.target.value)}
          />
        </div>
        {problem === null ? null : (
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertDescription className="text-foreground">{problem}</AlertDescription>
          </Alert>
        )}
        <AlertDialogFooter>
          <Button type="button" variant="outline" disabled={pending} onClick={() => onOpenChange(false)}>
            Keep game
          </Button>
          <Button type="button" variant="destructive" loading={pending} onClick={() => void end()}>
            {pending ? 'Ending…' : 'End game'}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

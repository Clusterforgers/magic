import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { questionWords } from './lobbyQuestions'
import type { Asking } from './useLobbyControls'

interface LobbyQuestionProps {
  asking: Asking
  /** The yes is on its way: the buttons wait and Escape does nothing. */
  busy: boolean
  /** Where focus goes when the button that asked has gone (a removed player's Remove): the seat list. */
  fallback(): HTMLElement | null
  onConfirm(): void
  onCancel(): void
  /** The dialog has finished closing. */
  onClosed(): void
}

/**
 * The question before Close, Leave and Remove, named by what it does: the safe answer is first and takes the focus, the
 * other says what it will do and to whom. A refused yes keeps the question up with the reason.
 */
export function LobbyQuestion({ asking, busy, fallback, onConfirm, onCancel, onClosed }: LobbyQuestionProps) {
  const words = questionWords(asking.question)
  const target = (): HTMLElement | null =>
    asking.after === 'opener' && asking.opener?.isConnected === true ? asking.opener : fallback()
  return (
    <AlertDialog
      open={asking.open}
      onOpenChange={(next) => {
        if (!next && !busy) onCancel()
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed()
      }}
    >
      <AlertDialogContent finalFocus={() => target() ?? true}>
        <AlertDialogTitle className="[overflow-wrap:anywhere]">{words.title}</AlertDialogTitle>
        <AlertDialogDescription className="[overflow-wrap:anywhere]">{words.description}</AlertDialogDescription>
        {asking.problem === null ? null : (
          <p role="alert" className="rounded-md bg-destructive/10 px-2 py-1 text-sm text-destructive">
            {asking.problem}
          </p>
        )}
        <AlertDialogFooter>
          <Button type="button" variant="outline" disabled={busy} onClick={onCancel}>
            {words.cancel}
          </Button>
          <Button type="button" variant="destructive" loading={busy} onClick={onConfirm}>
            {words.confirm}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

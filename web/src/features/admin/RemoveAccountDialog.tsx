import { CircleAlertIcon } from 'lucide-react'
import { useState } from 'react'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { useAuthStore } from '@/features/auth/context'
import { describeAccountError } from '@/features/auth/errors'
import type { AccountRow } from '@/protocol/generated'

interface RemoveAccountDialogProps {
  account: AccountRow
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The account is gone; the list drops its row. */
  onRemoved: () => void
}

/**
 * The question before an account is removed. It says plainly what goes: the account, every deck, the lobbies it
 * hosts and its place in the games it plays. The dialog traps focus, closes on Escape and starts on the safe button.
 */
export function RemoveAccountDialog({ account, open, onOpenChange, onRemoved }: RemoveAccountDialogProps) {
  const store = useAuthStore()
  const [pending, setPending] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  async function remove(): Promise<void> {
    if (pending) return
    setPending(true)
    setProblem(null)
    const result = await store.getState().removeAccount(account.id)
    setPending(false)
    if (result.ok) {
      onOpenChange(false)
      onRemoved()
      return
    }
    setProblem(describeAccountError(result.error, 'admin-remove').message)
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
        <AlertDialogTitle>{`Remove ${account.username}?`}</AlertDialogTitle>
        <AlertDialogDescription>
          {`This deletes the account of ${account.username} and all of their decks for good. The lobbies they host are closed, they leave the games they are in, and they can no longer sign in. This cannot be undone.`}
        </AlertDialogDescription>
        {problem === null ? null : (
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertDescription className="text-foreground">{problem}</AlertDescription>
          </Alert>
        )}
        <AlertDialogFooter>
          <Button type="button" variant="outline" disabled={pending} onClick={() => onOpenChange(false)}>
            Keep account
          </Button>
          <Button type="button" variant="destructive" loading={pending} onClick={() => void remove()}>
            {pending ? 'Removing…' : 'Remove account'}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

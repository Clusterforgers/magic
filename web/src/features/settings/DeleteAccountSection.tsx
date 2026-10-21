import { useNavigate } from '@tanstack/react-router'
import { useRef, useState } from 'react'
import type { FormEvent } from 'react'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useAuthStore } from '@/features/auth/context'
import { describeAccountError } from '@/features/auth/errors'
import { Field } from '@/features/auth/Field'
import { describedBy } from '@/features/auth/fieldIds'
import { showFlash } from '@/lib/flash'

interface DeleteAccountSectionProps {
  /** Called with true just before the account is deleted and with false if that did not work, so the page does not send a signed-out visitor to the sign-in form. */
  onDeleting: (deleting: boolean) => void
}

/**
 * The danger zone: what deleting does, and a button that asks for the password in a dialog before anything is sent. A
 * wrong password or a refusal keeps the question up with the reason; success goes home with a notice.
 */
export function DeleteAccountSection({ onDeleting }: DeleteAccountSectionProps) {
  const store = useAuthStore()
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)
  const [pending, setPending] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)
  const passwordRef = useRef<HTMLInputElement>(null)

  async function confirm(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    const password = passwordRef.current?.value ?? ''
    if (password === '') {
      setProblem('Enter your password to confirm.')
      passwordRef.current?.focus()
      return
    }
    setPending(true)
    setProblem(null)
    onDeleting(true)
    const result = await store.getState().deleteAccount(password)
    if (passwordRef.current !== null) passwordRef.current.value = ''
    setPending(false)
    if (result.ok) {
      setOpen(false)
      showFlash('Your account was deleted.')
      void navigate({ to: '/', replace: true })
      return
    }
    onDeleting(false)
    setProblem(describeAccountError(result.error, 'delete').message)
    passwordRef.current?.focus()
  }

  return (
    <section
      aria-labelledby="delete-account-title"
      className="surface grid gap-4 border-destructive/40 p-4 sm:p-6"
    >
      <div className="grid gap-1">
        <h2 id="delete-account-title" className="text-lg font-semibold tracking-tight">
          Delete account
        </h2>
        <p className="text-sm text-muted-foreground">
          This cannot be undone. Your decks are deleted and you are signed out everywhere. You leave your
          games: a seat you took in a lobby is given back, a lobby you host is closed, and a seat in a game
          that is running stays at the table for its link. Games keep the name you played under. The username
          can be taken again.
        </p>
      </div>
      <Button type="button" variant="destructive" className="w-full sm:w-fit" onClick={() => setOpen(true)}>
        Delete account…
      </Button>

      <AlertDialog
        open={open}
        onOpenChange={(next) => {
          if (pending) return
          setOpen(next)
          if (!next) setProblem(null)
        }}
      >
        <AlertDialogContent>
          <form
            method="post"
            noValidate
            aria-busy={pending}
            className="grid gap-4"
            onSubmit={(event) => void confirm(event)}
          >
            <AlertDialogTitle>Delete your account?</AlertDialogTitle>
            <AlertDialogDescription>
              Your decks are deleted for good and you are signed out everywhere. Type your password to
              confirm.
            </AlertDialogDescription>
            <Field id="delete-password" label="Password" rule={null} problem={problem}>
              <Input
                ref={passwordRef}
                id="delete-password"
                name="password"
                type="password"
                autoComplete="current-password"
                readOnly={pending}
                aria-invalid={problem !== null}
                aria-describedby={describedBy('delete-password', false)}
              />
            </Field>
            <AlertDialogFooter>
              <Button type="button" variant="outline" disabled={pending} onClick={() => setOpen(false)}>
                Keep account
              </Button>
              <Button type="submit" variant="destructive" loading={pending}>
                {pending ? 'Deleting…' : 'Delete account'}
              </Button>
            </AlertDialogFooter>
          </form>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  )
}

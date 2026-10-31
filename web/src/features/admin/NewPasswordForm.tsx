import { CircleAlertIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { FormEvent, KeyboardEvent } from 'react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { useAuthStore } from '@/features/auth/context'
import { describeAccountError } from '@/features/auth/errors'
import { passwordProblem } from '@/features/auth/rules'
import type { AccountRow } from '@/protocol/generated'
import { PasswordField } from './PasswordField'
import { usePasswordControl } from './usePasswordControl'

interface NewPasswordFormProps {
  account: AccountRow
  /** The server took the password; the row shows it once. */
  onSet: (password: string) => void
  onCancel: () => void
}

/**
 * The form under an account's row that sets a new password for a friend who forgot theirs: the same field and
 * generator as making an account. Sending it ends the account's sessions. The field is read once at submit and kept
 * nowhere; Escape or "Cancel" closes the form.
 */
export function NewPasswordForm({ account, onSet, onCancel }: NewPasswordFormProps) {
  const store = useAuthStore()
  const control = usePasswordControl()
  const [pending, setPending] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)
  const [failure, setFailure] = useState<string | null>(null)
  const alertRef = useRef<HTMLDivElement>(null)
  const fieldId = `reset-password-${account.id}`

  const { focus, read, clear } = control
  useEffect(() => focus(), [focus])

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    // Nothing is trimmed or changed: the server compares exactly what was typed.
    const password = read()
    const wrong = passwordProblem(password)
    if (wrong !== null) {
      setProblem(wrong)
      focus()
      return
    }
    setPending(true)
    setProblem(null)
    setFailure(null)
    const result = await store.getState().resetAccountPassword(account.id, password)
    setPending(false)
    if (result.ok) {
      clear()
      onSet(password)
      return
    }
    const shown = describeAccountError(result.error, 'admin-reset')
    if (shown.field === 'password') {
      setProblem(shown.message)
      // The field stays as typed, so a refused password is changed and sent again.
      focus()
    } else {
      setFailure(shown.message)
      window.requestAnimationFrame(() => alertRef.current?.focus())
    }
  }

  function keyDown(event: KeyboardEvent<HTMLFormElement>): void {
    if (event.key === 'Escape' && !pending) onCancel()
  }

  return (
    <form
      // Never GET: if a submit ever got past `preventDefault`, the password must not end up in the address.
      method="post"
      noValidate
      aria-label={`New password for ${account.username}`}
      aria-busy={pending}
      className="grid gap-4 rounded-lg border border-border bg-muted/40 p-4"
      onSubmit={(event) => void submit(event)}
      onKeyDown={keyDown}
    >
      {failure === null ? null : (
        <Alert ref={alertRef} tabIndex={-1} variant="destructive" className="outline-none">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not set the password</AlertTitle>
          <AlertDescription>{failure}</AlertDescription>
        </Alert>
      )}
      <PasswordField
        id={fieldId}
        label="New password"
        control={control}
        problem={problem}
        pending={pending}
        onEdit={(value) => {
          if (problem !== null) setProblem(passwordProblem(value))
        }}
        onGenerated={() => setProblem(null)}
      />
      <div className="flex flex-wrap gap-2">
        <Button type="submit" loading={pending}>
          {pending ? 'Setting the password…' : 'Set password'}
        </Button>
        <Button type="button" variant="ghost" disabled={pending} onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  )
}

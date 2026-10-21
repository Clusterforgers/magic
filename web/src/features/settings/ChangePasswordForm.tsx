import { CircleAlertIcon, CircleCheckIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useAuthStore } from '@/features/auth/context'
import { describeAccountError } from '@/features/auth/errors'
import { Field } from '@/features/auth/Field'
import { describedBy } from '@/features/auth/fieldIds'
import { PASSWORD_RULE, passwordProblem } from '@/features/auth/rules'

interface Problems {
  current: string | null
  next: string | null
  confirm: string | null
  form: string | null
}

const NO_PROBLEMS: Problems = { current: null, next: null, confirm: null, form: null }

/**
 * The change-password form, in a card. The fields are uncontrolled, so a password lives only in its input while it is
 * typed: it is read once at submit and sent. The current password is emptied after every submit, the other two once
 * the change went through. The server judges every rule; the form only spares a round trip for the obvious ones.
 */
export function ChangePasswordForm() {
  const store = useAuthStore()
  const [pending, setPending] = useState(false)
  const [done, setDone] = useState(false)
  const [problems, setProblems] = useState<Problems>(NO_PROBLEMS)
  const [focusRound, setFocusRound] = useState(0)
  const formRef = useRef<HTMLFormElement>(null)
  const currentRef = useRef<HTMLInputElement>(null)
  const nextRef = useRef<HTMLInputElement>(null)
  const confirmRef = useRef<HTMLInputElement>(null)
  const alertRef = useRef<HTMLDivElement>(null)

  // After a render that shows problems: focus the first field that is wrong, else the alert about the whole form.
  useEffect(() => {
    if (focusRound === 0) return
    const wrong = formRef.current?.querySelector<HTMLElement>('[aria-invalid="true"]')
    ;(wrong ?? alertRef.current)?.focus()
  }, [focusRound])

  const showProblems = (shown: Problems): void => {
    setProblems(shown)
    setFocusRound((round) => round + 1)
  }

  const empty = (...inputs: (HTMLInputElement | null)[]): void => {
    for (const input of inputs) if (input !== null) input.value = ''
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    // Nothing is trimmed or changed: the server compares exactly what was typed.
    const current = currentRef.current?.value ?? ''
    const next = nextRef.current?.value ?? ''
    const again = confirmRef.current?.value ?? ''
    const wrong: Problems = {
      current: current === '' ? 'Enter your current password.' : null,
      next: passwordProblem(next),
      confirm: next === again ? null : 'The two new passwords are not the same.',
      form: null,
    }
    if (wrong.current !== null || wrong.next !== null || wrong.confirm !== null) {
      showProblems(wrong)
      return
    }
    setPending(true)
    setDone(false)
    setProblems(NO_PROBLEMS)
    const result = await store.getState().changePassword(current, next)
    empty(currentRef.current)
    setPending(false)
    if (result.ok) {
      empty(nextRef.current, confirmRef.current)
      setDone(true)
      return
    }
    const shown = describeAccountError(result.error, 'password')
    const field = shown.field === 'current' ? 'current' : shown.field === 'password' ? 'next' : 'form'
    showProblems({ ...NO_PROBLEMS, [field]: shown.message })
  }

  return (
    <form
      ref={formRef}
      // Never GET: if a submit ever got past `preventDefault`, the passwords must not end up in the address.
      method="post"
      noValidate
      aria-labelledby="change-password-title"
      aria-busy={pending}
      className="surface grid gap-6 p-4 sm:p-6"
      onSubmit={(event) => void submit(event)}
    >
      <div className="grid gap-1">
        <h2 id="change-password-title" className="text-lg font-semibold tracking-tight">
          Change password
        </h2>
        <p className="text-sm text-muted-foreground">
          Every other browser you are signed in on is signed out. This one stays signed in.
        </p>
      </div>

      {problems.form === null ? null : (
        <Alert ref={alertRef} tabIndex={-1} variant="destructive" className="outline-none">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not change the password</AlertTitle>
          <AlertDescription>{problems.form}</AlertDescription>
        </Alert>
      )}

      <Field id="current-password" label="Current password" rule={null} problem={problems.current}>
        <Input
          ref={currentRef}
          id="current-password"
          name="current-password"
          type="password"
          autoComplete="current-password"
          readOnly={pending}
          aria-invalid={problems.current !== null}
          aria-describedby={describedBy('current-password', false)}
        />
      </Field>

      <Field id="new-password" label="New password" rule={PASSWORD_RULE} problem={problems.next}>
        <Input
          ref={nextRef}
          id="new-password"
          name="new-password"
          type="password"
          autoComplete="new-password"
          readOnly={pending}
          aria-invalid={problems.next !== null}
          aria-describedby={describedBy('new-password')}
        />
      </Field>

      <Field id="confirm-password" label="New password again" rule={null} problem={problems.confirm}>
        <Input
          ref={confirmRef}
          id="confirm-password"
          name="confirm-password"
          type="password"
          autoComplete="new-password"
          readOnly={pending}
          aria-invalid={problems.confirm !== null}
          aria-describedby={describedBy('confirm-password', false)}
        />
      </Field>

      {/* Always in the page, so the words of a success are announced when they arrive. */}
      <div role="status" className="empty:hidden">
        {done ? (
          <p className="flex items-start gap-2 text-sm">
            <CircleCheckIcon aria-hidden="true" className="size-4 shrink-0 translate-y-0.5 text-success" />
            <span>Password changed. Your other sessions are signed out.</span>
          </p>
        ) : null}
      </div>

      <Button type="submit" size="lg" loading={pending} className="w-full">
        {pending ? 'Changing the password…' : 'Change password'}
      </Button>
    </form>
  )
}

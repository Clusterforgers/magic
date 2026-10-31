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
import { passwordProblem, USERNAME_RULE, usernameProblem } from '@/features/auth/rules'
import { CredentialsPanel } from './CredentialsPanel'
import { PasswordField } from './PasswordField'
import { usePasswordControl } from './usePasswordControl'

interface Problems {
  username: string | null
  password: string | null
  form: string | null
}

const NO_PROBLEMS: Problems = { username: null, password: null, form: null }

/** The account just made, kept in memory only until the admin is done copying it. */
interface Made {
  username: string
  password: string
}

/**
 * The form that makes an account for a friend, in a card. The fields are uncontrolled, so a typed password lives only in
 * its input and is read once at submit. "Make a password" fills a random one and shows it in plain text so it can be
 * copied; the password is never stored in the browser. After a success the address, username and password are shown once,
 * with a copy button, until the admin dismisses them or makes the next account.
 */
export function NewAccountForm() {
  const store = useAuthStore()
  const [pending, setPending] = useState(false)
  const [problems, setProblems] = useState<Problems>(NO_PROBLEMS)
  const [focusRound, setFocusRound] = useState(0)
  const [made, setMade] = useState<Made | null>(null)
  const formRef = useRef<HTMLFormElement>(null)
  const usernameRef = useRef<HTMLInputElement>(null)
  const alertRef = useRef<HTMLDivElement>(null)
  const passwordControl = usePasswordControl()

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

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    // Nothing is trimmed or changed: the server compares exactly what was typed.
    const username = usernameRef.current?.value ?? ''
    const password = passwordControl.read()
    const wrong = { username: usernameProblem(username), password: passwordProblem(password), form: null }
    if (wrong.username !== null || wrong.password !== null) {
      showProblems(wrong)
      return
    }
    setPending(true)
    setMade(null)
    setProblems(NO_PROBLEMS)
    const result = await store.getState().createAccount({ username, password })
    setPending(false)
    if (result.ok) {
      if (usernameRef.current !== null) usernameRef.current.value = ''
      passwordControl.clear()
      // The panel with the new credentials takes the focus and scrolls itself into view.
      setMade({ username: result.account.username, password })
      return
    }
    // The fields stay as they were, so a taken name is changed and sent again without typing the rest.
    const shown = describeAccountError(result.error, 'admin')
    showProblems({ ...NO_PROBLEMS, [shown.field === 'current' ? 'form' : shown.field]: shown.message })
  }

  return (
    <form
      ref={formRef}
      // Never GET: if a submit ever got past `preventDefault`, the credentials must not end up in the address.
      method="post"
      noValidate
      aria-labelledby="made-account-title"
      aria-busy={pending}
      className="surface grid gap-6 p-4 sm:p-6"
      onSubmit={(event) => void submit(event)}
    >
      <div className="grid gap-1">
        <h2 id="made-account-title" className="text-lg font-semibold tracking-tight">
          Make an account
        </h2>
        <p className="text-sm text-muted-foreground">
          Your friend signs in with the username and password you set here. Give them the address too.
        </p>
      </div>

      {problems.form === null ? null : (
        <Alert ref={alertRef} tabIndex={-1} variant="destructive" className="outline-none">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not make the account</AlertTitle>
          <AlertDescription>{problems.form}</AlertDescription>
        </Alert>
      )}

      <Field id="made-username" label="Username" rule={USERNAME_RULE} problem={problems.username}>
        <Input
          ref={usernameRef}
          id="made-username"
          name="made-username"
          autoComplete="off"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          readOnly={pending}
          aria-invalid={problems.username !== null}
          aria-describedby={describedBy('made-username')}
          onChange={(event) => {
            if (problems.username !== null) {
              setProblems((was) => ({ ...was, username: usernameProblem(event.target.value) }))
            }
          }}
        />
      </Field>

      <PasswordField
        id="made-password"
        control={passwordControl}
        problem={problems.password}
        pending={pending}
        onEdit={(value) => {
          if (problems.password !== null) setProblems((was) => ({ ...was, password: passwordProblem(value) }))
        }}
        onGenerated={() => setProblems((was) => ({ ...was, password: null }))}
      />

      <Button type="submit" size="lg" loading={pending} className="w-full">
        {pending ? 'Making the account…' : 'Make account'}
      </Button>

      {/* Always in the page, so the words of a success are announced when they arrive. */}
      <div role="status" className="empty:hidden">
        {made === null ? null : (
          <p className="flex items-start gap-2 text-sm">
            <CircleCheckIcon aria-hidden="true" className="size-4 shrink-0 translate-y-0.5 text-success" />
            <span>Account made.</span>
          </p>
        )}
      </div>

      {made === null ? null : (
        <CredentialsPanel
          label="The new account"
          username={made.username}
          password={made.password}
          note="This is the only time the password is shown. Tell your friend to change it under Account settings."
          onDone={() => setMade(null)}
        />
      )}
    </form>
  )
}

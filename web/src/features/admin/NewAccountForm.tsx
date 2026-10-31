import { CircleAlertIcon, CircleCheckIcon, DicesIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useAuthStore } from '@/features/auth/context'
import { describeAccountError } from '@/features/auth/errors'
import { Field } from '@/features/auth/Field'
import { describedBy } from '@/features/auth/fieldIds'
import { PASSWORD_RULE, passwordProblem, USERNAME_RULE, usernameProblem } from '@/features/auth/rules'
import { generatePassword } from './password'

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
 * copied; the password is never stored in the browser. After a success the username and password are shown once, with a
 * copy button, until the admin dismisses them or makes the next account.
 */
export function NewAccountForm() {
  const store = useAuthStore()
  const [pending, setPending] = useState(false)
  const [problems, setProblems] = useState<Problems>(NO_PROBLEMS)
  const [focusRound, setFocusRound] = useState(0)
  const [revealed, setRevealed] = useState(false)
  const [made, setMade] = useState<Made | null>(null)
  const formRef = useRef<HTMLFormElement>(null)
  const usernameRef = useRef<HTMLInputElement>(null)
  const passwordRef = useRef<HTMLInputElement>(null)
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

  function makePassword(): void {
    if (passwordRef.current === null) return
    passwordRef.current.value = generatePassword()
    setRevealed(true)
    setProblems((was) => ({ ...was, password: null }))
    passwordRef.current.focus()
  }

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    if (pending) return
    // Nothing is trimmed or changed: the server compares exactly what was typed.
    const username = usernameRef.current?.value ?? ''
    const password = passwordRef.current?.value ?? ''
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
      if (passwordRef.current !== null) passwordRef.current.value = ''
      setRevealed(false)
      setMade({ username: result.account.username, password })
      usernameRef.current?.focus()
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
          The account is made at once and nobody is signed in by it. Give your friend the username and
          password.
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

      <Field id="made-password" label="Password" rule={PASSWORD_RULE} problem={problems.password}>
        <Input
          ref={passwordRef}
          id="made-password"
          name="made-password"
          type={revealed ? 'text' : 'password'}
          autoComplete="off"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          readOnly={pending}
          aria-invalid={problems.password !== null}
          aria-describedby={describedBy('made-password')}
          className={revealed ? 'font-mono' : undefined}
          onChange={(event) => {
            if (problems.password !== null) {
              setProblems((was) => ({ ...was, password: passwordProblem(event.target.value) }))
            }
          }}
        />
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="outline" size="sm" disabled={pending} onClick={makePassword}>
            <DicesIcon aria-hidden="true" />
            Make a password
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-pressed={revealed}
            onClick={() => setRevealed((shown) => !shown)}
          >
            {revealed ? 'Hide the password' : 'Show the password'}
          </Button>
        </div>
      </Field>

      <Button type="submit" size="lg" loading={pending} className="w-full">
        {pending ? 'Making the account…' : 'Create account'}
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
        <section
          aria-label="The new account"
          className="grid gap-3 rounded-lg border border-success/40 bg-success/12 p-4 text-sm"
        >
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1">
            <dt className="text-muted-foreground">Username</dt>
            <dd className="font-mono break-all select-all">{made.username}</dd>
            <dt className="text-muted-foreground">Password</dt>
            <dd className="font-mono break-all select-all">{made.password}</dd>
          </dl>
          <p className="text-muted-foreground">
            This is the only time the password is shown. Tell your friend to change it under Account settings.
          </p>
          <div className="flex flex-wrap gap-2">
            <CopyButton
              text={`Username: ${made.username}\nPassword: ${made.password}`}
              label="Copy the username and password"
            >
              Copy both
            </CopyButton>
            <Button type="button" variant="ghost" size="sm" onClick={() => setMade(null)}>
              Done
            </Button>
          </div>
        </section>
      )}
    </form>
  )
}

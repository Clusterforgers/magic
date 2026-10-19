import { Link } from '@tanstack/react-router'
import { CircleAlertIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { FormEvent } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAuthStore } from './context'
import { describeAccountError } from './errors'
import type { AuthMode } from './errors'
import { PASSWORD_RULE, passwordProblem, USERNAME_RULE, usernameProblem } from './rules'

interface Problems {
  username: string | null
  password: string | null
  form: string | null
}

const NO_PROBLEMS: Problems = { username: null, password: null, form: null }

const WORDS = {
  login: {
    title: 'Sign in',
    intro: 'Sign in to find your games again and rejoin a seat without its link.',
    submit: 'Sign in',
    pending: 'Signing in…',
    other: { to: '/register', question: 'No account yet?', action: 'Create one' },
  },
  register: {
    title: 'Create an account',
    intro:
      'An account lists your games and lets you rejoin a seat without its link. You can still play as a guest with a seat link.',
    submit: 'Create account',
    pending: 'Creating the account…',
    other: { to: '/login', question: 'Already have an account?', action: 'Sign in' },
  },
} as const

interface AuthFormProps {
  mode: AuthMode
  /** Where the visitor was headed, kept when they switch between the two forms. */
  next: string | undefined
}

/**
 * The sign-in and register form. The fields are uncontrolled, so the password lives only in the input while it is
 * typed: it is read once at submit, sent, and the field is emptied as soon as the answer is in. State holds only
 * the words of the problems.
 */
export function AuthForm({ mode, next }: AuthFormProps) {
  const store = useAuthStore()
  const words = WORDS[mode]
  const [pending, setPending] = useState(false)
  const [problems, setProblems] = useState<Problems>(NO_PROBLEMS)
  const [focusRound, setFocusRound] = useState(0)
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
    setProblems(NO_PROBLEMS)
    const { login, register } = store.getState()
    const result = await (mode === 'login' ? login : register)({ username, password })
    if (passwordRef.current !== null) passwordRef.current.value = ''
    setPending(false)
    if (result.ok) return // the page moves on by itself once the store says signed in
    const shown = describeAccountError(result.error, mode)
    showProblems({ ...NO_PROBLEMS, [shown.field]: shown.message })
  }

  const usernameId = `${mode}-username`
  const passwordId = `${mode}-password`
  const describedBy = (id: string, problem: string | null): string =>
    problem === null ? `${id}-hint` : `${id}-hint ${id}-problem`

  return (
    <form
      ref={formRef}
      // Never GET: if a submit ever got past `preventDefault`, the credentials must not end up in the address.
      method="post"
      noValidate
      aria-labelledby={`${mode}-title`}
      aria-busy={pending}
      className="grid gap-5"
      onSubmit={(event) => void submit(event)}
    >
      <div>
        <h1 id={`${mode}-title`} className="text-2xl font-semibold tracking-tight">
          {words.title}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground">{words.intro}</p>
      </div>

      {problems.form === null ? null : (
        <Alert ref={alertRef} tabIndex={-1} variant="destructive" className="outline-none">
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">{problems.form}</AlertDescription>
        </Alert>
      )}

      <div className="grid gap-1.5">
        <Label htmlFor={usernameId}>Username</Label>
        <Input
          ref={usernameRef}
          id={usernameId}
          name="username"
          autoComplete="username"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          readOnly={pending}
          aria-invalid={problems.username !== null}
          aria-describedby={describedBy(usernameId, problems.username)}
          onChange={(event) => {
            if (problems.username !== null) {
              setProblems((was) => ({ ...was, username: usernameProblem(event.target.value) }))
            }
          }}
        />
        <p id={`${usernameId}-hint`} className="text-xs text-muted-foreground">
          {USERNAME_RULE}
        </p>
        {problems.username === null ? null : (
          <p id={`${usernameId}-problem`} className="text-sm text-destructive">
            {problems.username}
          </p>
        )}
      </div>

      <div className="grid gap-1.5">
        <Label htmlFor={passwordId}>Password</Label>
        <Input
          ref={passwordRef}
          id={passwordId}
          name="password"
          type="password"
          autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
          readOnly={pending}
          aria-invalid={problems.password !== null}
          aria-describedby={describedBy(passwordId, problems.password)}
          onChange={(event) => {
            if (problems.password !== null) {
              setProblems((was) => ({ ...was, password: passwordProblem(event.target.value) }))
            }
          }}
        />
        <p id={`${passwordId}-hint`} className="text-xs text-muted-foreground">
          {PASSWORD_RULE}
        </p>
        {problems.password === null ? null : (
          <p id={`${passwordId}-problem`} className="text-sm text-destructive">
            {problems.password}
          </p>
        )}
      </div>

      <div>
        <Button type="submit" size="lg" loading={pending}>
          {pending ? words.pending : words.submit}
        </Button>
      </div>

      <p className="text-sm text-muted-foreground">
        {words.other.question}{' '}
        <Link
          to={words.other.to}
          search={next === undefined ? {} : { next }}
          className="font-medium text-foreground underline underline-offset-3"
        >
          {words.other.action}
        </Link>
      </p>
    </form>
  )
}

import { getRouteApi, Link } from '@tanstack/react-router'
import { CircleAlertIcon } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import type { FormEvent, ReactNode } from 'react'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { nameProblem } from '@/features/create/form'
import { usePageTitle } from '@/hooks/usePageTitle'
import { codeText } from '@/lib/joinCode'
import { api as sharedApi } from '@/net/api'
import type { Api } from '@/net/api'
import { isJoinCode, normalizeCode } from './format'
import { loadGuestName, saveGuestName } from './guestName'
import { CODE_SHAPE } from './joinErrors'
import { useJoinGame } from './useJoinGame'
import type { JoinGame } from './useJoinGame'

const route = getRouteApi('/join/$code')

const NAME_HINT = 'Choose the name the other players see at the table.'

/** The code the page is about, large enough to check against what the host said. */
function CodeLine({ shown }: { shown: string }) {
  return (
    <dl className="grid gap-1">
      <dt className="text-sm text-muted-foreground">Join code</dt>
      <dd data-slot="join-code" className="font-mono text-2xl tracking-widest">
        {shown}
      </dd>
    </dl>
  )
}

/**
 * A page with nothing left to join: what is wrong in a sentence, and the two ways on. Focus goes to Play, the first of
 * them, and the sentence is its description so a screen reader says both.
 */
function DeadEnd({ title, children }: { title: string; children: ReactNode }) {
  const play = useRef<HTMLAnchorElement>(null)
  const said = useId()
  useEffect(() => {
    play.current?.focus()
  }, [])
  return (
    <>
      <Alert variant="destructive">
        <CircleAlertIcon aria-hidden="true" />
        <AlertTitle>{title}</AlertTitle>
        <AlertDescription id={said}>{children}</AlertDescription>
      </Alert>
      <div className="flex flex-wrap gap-2">
        <Link ref={play} to="/" aria-describedby={said} className={buttonVariants({ size: 'lg' })}>
          Play
        </Link>
        <Link to="/new" className={buttonVariants({ size: 'lg', variant: 'outline' })}>
          Create a game
        </Link>
      </div>
    </>
  )
}

interface FormProps {
  /** The code as the server reads it. */
  code: string
  join: JoinGame
  /** The signed-in account's name, or null for a guest, who gives a name here. */
  account: string | null
}

/** The name (for a guest) and the one Join button. Focus starts on the first thing to do: the name, or Join. */
function JoinForm({ code, join, account }: FormProps) {
  const guest = account === null
  const [name, setName] = useState(loadGuestName)
  const [problem, setProblem] = useState<string | null>(null)
  const nameField = useRef<HTMLInputElement>(null)
  const button = useRef<HTMLButtonElement>(null)
  const described = useId()
  const nameId = useId()

  useEffect(() => {
    const first = guest ? nameField.current : button.current
    first?.focus()
  }, [guest])

  function submit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault()
    // Enter in the name field submits even while Join waits, so a request that is out stops here.
    if (join.busy) return
    if (!guest) {
      join.startCode(code, null)
      return
    }
    const wrong = nameProblem(name)
    setProblem(wrong)
    if (wrong !== null) {
      nameField.current?.focus()
      return
    }
    const trimmed = name.trim()
    saveGuestName(trimmed)
    join.startCode(code, null, trimmed)
  }

  return (
    <form
      noValidate
      aria-label="Join a game"
      aria-busy={join.busy || undefined}
      onSubmit={submit}
      className="grid gap-4"
    >
      {guest ? (
        <div className="grid gap-2">
          <Label htmlFor={nameId}>Your name</Label>
          <Input
            ref={nameField}
            id={nameId}
            name="name"
            value={name}
            readOnly={join.busy}
            autoComplete="nickname"
            aria-invalid={problem === null ? undefined : true}
            aria-describedby={described}
            onChange={(event) => {
              setName(event.target.value)
              setProblem(null)
            }}
          />
          <p
            id={described}
            className={problem === null ? 'text-sm text-muted-foreground' : 'text-sm text-destructive'}
          >
            {problem ?? NAME_HINT}
          </p>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">{`You join as ${account}.`}</p>
      )}
      {join.codeProblem === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not join</AlertTitle>
          <AlertDescription>{join.codeProblem}</AlertDescription>
        </Alert>
      )}
      <Button ref={button} type="submit" size="lg" loading={join.busy}>
        Join game
      </Button>
    </form>
  )
}

/** What `JoinCodePage` takes. */
export interface JoinCodePageProps {
  /** The code as the address holds it: any case, with or without the hyphen. */
  code: string
  /** The client; the shared one by default. */
  api?: Pick<Api, 'joinGame' | 'joinByCode'>
}

/**
 * `/join/$code`: where an invite link leads. It shows the code in its normal form and one Join button; a guest also
 * gives a name here, and a signed-in visitor joins as the account. A code that cannot be one is said so without asking
 * the server. A code no open game has, or a game that is full or started, ends in a sentence and the way back to Play or
 * to Create a game; a failure that sending again could mend stays on the form.
 */
export function JoinCodePage({ code, api = sharedApi }: JoinCodePageProps) {
  usePageTitle('Join a game')
  const normal = normalizeCode(code)
  const status = useAuth((state) => state.status)
  const username = useAuth((state) => state.user?.username ?? null)
  const join = useJoinGame(api)
  const spent = join.codeFailure?.final === true ? join.codeFailure.text : null

  let body: ReactNode
  if (!isJoinCode(normal)) {
    body = (
      <DeadEnd title="That is not a join code">
        {`${CODE_SHAPE} Ask the host for the link again, or type the code on the Play page.`}
      </DeadEnd>
    )
  } else if (spent !== null) {
    body = (
      <>
        <CodeLine shown={codeText(normal)} />
        <DeadEnd title="Could not join">{spent}</DeadEnd>
      </>
    )
  } else {
    body = (
      <>
        <CodeLine shown={codeText(normal)} />
        {status === 'loading' ? (
          <Loading label="Checking your sign-in…">
            <Skeleton className="h-11 w-full" />
          </Loading>
        ) : (
          <JoinForm code={normal} join={join} account={status === 'signed-in' ? username : null} />
        )}
      </>
    )
  }

  return (
    <PageShell width="narrow" className="grid gap-4">
      <h1 className="text-2xl font-semibold tracking-tight">Join a game</h1>
      <div className="surface grid gap-4 p-6">{body}</div>
    </PageShell>
  )
}

/** The component of `/join/$code`: the code is the last part of the address. */
export function JoinCodeRoute() {
  const { code } = route.useParams()
  return <JoinCodePage code={code} />
}

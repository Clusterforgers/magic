import { Link } from '@tanstack/react-router'
import { LoaderCircle } from 'lucide-react'
import type { ElementType, ReactNode } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { UNREACHABLE_AFTER } from './connection'
import type { Connection } from './connection'

interface ConnectionNoticeProps {
  connection: Exclude<Connection, { kind: 'open' }>
  /** The only thing on the page, so its title is the page's main heading. */
  standalone: boolean
  /** REST says the game does not exist, which makes a stale link the likely culprit. */
  missing: boolean
  /** What opened the socket: a seat link (the default) or the signed-in session, which has other ways to go wrong. */
  via?: 'link' | 'session'
  /** Starts again from the first attempt. */
  onRetry(): void
}

interface Words {
  title: string
  body: ReactNode
  /** The label of the button that tries again, if there is one. */
  retry?: string
  /** Waiting will not help: announce it as an alert, with the title in the warning colour when `bad`. */
  alert: boolean
  bad?: boolean
}

function unreachableBody(via: 'link' | 'session', missing: boolean): ReactNode {
  if (via === 'session') {
    return (
      <>
        <p>
          The page tried {UNREACHABLE_AFTER} times and is still trying in the background. The server may be
          down or restarting, your network may be offline, you may have been signed out, or no seat in this
          game is saved to your account.
        </p>
        <p className="mt-2">
          {missing
            ? 'The server also says this game does not exist. A game that has finished is only kept in My games as a record, and one that nobody has joined for a long time is removed.'
            : 'If you have a seat link for this game, open it once while signed in and the seat is saved to your account.'}
        </p>
      </>
    )
  }
  return (
    <>
      <p>
        The page tried {UNREACHABLE_AFTER} times and is still trying in the background. The server may be down
        or restarting, your network may be offline, or this seat link may be wrong or out of date.
      </p>
      {missing ? (
        <p className="mt-2">
          The server also says this game does not exist, so the link is probably out of date. Ask the host for
          a new one.
        </p>
      ) : null}
    </>
  )
}

function wordsFor(
  connection: ConnectionNoticeProps['connection'],
  missing: boolean,
  via: 'link' | 'session',
): Words {
  switch (connection.kind) {
    case 'connecting':
      return { title: 'Connecting to the game…', body: null, alert: false }
    case 'reconnecting':
      return {
        title: `Connection lost. Reconnecting… (attempt ${connection.attempt})`,
        body: 'The game waits for you. This page will carry on by itself once the connection is back.',
        alert: false,
      }
    case 'unreachable':
      return {
        title: 'Cannot reach the game',
        body: unreachableBody(via, missing),
        retry: 'Try again now',
        alert: true,
        bad: true,
      }
    case 'replaced':
      return {
        title: 'This seat is open somewhere else',
        body: connection.reason ?? 'Another window or tab took this seat over.',
        retry: 'Use this window instead',
        alert: true,
      }
    case 'stopped':
      return {
        title: 'The connection ended',
        body: connection.reason ?? 'The server closed the connection.',
        retry: 'Connect again',
        alert: true,
        bad: true,
      }
  }
}

/** Says where the connection stands when it is not open, and offers a way forward when waiting will not help. */
export function ConnectionNotice({
  connection,
  standalone,
  missing,
  via = 'link',
  onRetry,
}: ConnectionNoticeProps) {
  const { title, body, retry, alert, bad } = wordsFor(connection, missing, via)
  const Heading: ElementType = standalone ? 'h1' : 'h2'
  const heading = (
    <Heading
      className={cn(
        standalone ? 'text-2xl font-semibold tracking-tight' : 'font-medium',
        bad === true && 'text-destructive',
      )}
    >
      {title}
    </Heading>
  )
  if (!alert) {
    return (
      <div role="status" className="flex items-start gap-3 rounded-xl bg-muted p-4 text-sm">
        <LoaderCircle
          aria-hidden="true"
          className="mt-0.5 size-4 shrink-0 animate-spin motion-reduce:animate-none"
        />
        <div className="grid gap-1">
          {heading}
          {body === null ? null : <p className="text-muted-foreground">{body}</p>}
        </div>
      </div>
    )
  }
  return (
    <Alert variant={bad === true ? 'destructive' : 'default'} className="gap-2 p-4">
      {heading}
      <AlertDescription className="text-foreground">{body}</AlertDescription>
      <div className="mt-2 flex flex-wrap gap-2">
        {retry === undefined ? null : (
          <Button type="button" onClick={onRetry}>
            {retry}
          </Button>
        )}
        {connection.kind === 'unreachable' && via === 'session' ? (
          <Link to="/games" className={buttonVariants({ variant: 'outline' })}>
            My games
          </Link>
        ) : null}
        {connection.kind === 'unreachable' ? (
          <Link to="/" className={buttonVariants({ variant: 'outline' })}>
            Create a new game
          </Link>
        ) : null}
      </div>
    </Alert>
  )
}

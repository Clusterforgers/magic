import { LoaderCircle } from 'lucide-react'
import type { ElementType, ReactNode } from 'react'
import { Link } from '@tanstack/react-router'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import type { Connection } from './connection'
import { UNREACHABLE_AFTER } from './connection'

interface ConnectionNoticeProps {
  connection: Exclude<Connection, { kind: 'open' }>
  /** The only thing on the page, so its title is the page's main heading. */
  standalone: boolean
  /** REST says the game does not exist, which makes a stale link the likely culprit. */
  missing: boolean
  /** Starts again from the first attempt. */
  onRetry(): void
}

interface Words {
  title: string
  body: ReactNode
  /** The label of the button that tries again, if there is one. */
  retry?: string
  alert: boolean
}

function wordsFor(connection: ConnectionNoticeProps['connection'], missing: boolean): Words {
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
        body: (
          <>
            <p>
              {`The page tried ${UNREACHABLE_AFTER} times and is still trying in the background. The server may be down or restarting, your network may be offline, or this seat link may be wrong or out of date.`}
            </p>
            {missing ? (
              <p className="mt-2">
                The server also says this game does not exist, so the link is probably out of date. Ask the
                host for a new one.
              </p>
            ) : null}
          </>
        ),
        retry: 'Try again now',
        alert: true,
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
      }
  }
}

/** Says where the connection stands when it is not open, and offers a way forward when waiting will not help. */
export function ConnectionNotice({ connection, standalone, missing, onRetry }: ConnectionNoticeProps) {
  const { title, body, retry, alert } = wordsFor(connection, missing)
  const Heading: ElementType = standalone ? 'h1' : 'h2'
  const heading = (
    <Heading className={standalone ? 'text-xl font-semibold tracking-tight' : 'font-medium'}>{title}</Heading>
  )
  if (!alert) {
    return (
      <div role="status" className="flex items-start gap-3 rounded-xl bg-muted p-4 text-sm">
        <LoaderCircle aria-hidden="true" className="mt-0.5 size-4 shrink-0 animate-spin" />
        <div className="grid gap-1">
          {heading}
          {body === null ? null : <p className="text-muted-foreground">{body}</p>}
        </div>
      </div>
    )
  }
  return (
    <Alert variant="destructive" className="gap-2 p-4">
      {heading}
      <AlertDescription className="text-foreground">{body}</AlertDescription>
      <div className="mt-2 flex flex-wrap gap-2">
        {retry === undefined ? null : (
          <Button type="button" onClick={onRetry}>
            {retry}
          </Button>
        )}
        {connection.kind === 'unreachable' ? (
          <Link to="/" className={buttonVariants({ variant: 'outline' })}>
            Create a new game
          </Link>
        ) : null}
      </div>
    </Alert>
  )
}

import { Link } from '@tanstack/react-router'
import { Circle, CircleCheck } from 'lucide-react'
import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useGame, usePlay } from './gameContext'
import { Invites } from './Invites'
import { seatName } from './useGameInfo'
import { useSeatAccounts } from './useSeatAccounts'

/** Who is at the table before the game starts: the seats, who is connected, and this seat's Ready. */
export function Lobby() {
  const { game, session, lookup } = useGame()
  const { seat, seatCount, status } = usePlay(
    useShallow((state) => ({ seat: state.seat, seatCount: state.seatCount, status: state.status })),
  )
  const [ready, setReady] = useState(false)
  const [refused, setRefused] = useState(false)
  const accounts = useSeatAccounts(game, seat, `${status}|${lookup.game?.connected.join(',') ?? ''}`)

  const seats = seatCount ?? lookup.game?.seats.length ?? 0
  const rows = Array.from({ length: seats }, (_, index) => {
    const you = index === seat
    // REST lists who is connected; this page's own socket is the better witness for itself.
    const here = you ? status === 'open' : (lookup.game?.connected.includes(index) ?? false)
    return { index, you, here, name: seatName(lookup, index), account: accounts.bySeat.get(index) ?? null }
  })
  const present = rows.filter((row) => row.here).length

  function sayReady(): void {
    const sent = session.ready()
    setReady(sent)
    setRefused(!sent)
  }

  return (
    <div className="mx-auto grid w-full max-w-2xl gap-6 px-4 py-6 sm:px-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Waiting for players</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          The game starts as soon as every seat has joined and pressed Ready.
        </p>
      </div>

      <section aria-labelledby="seats-title" className="grid gap-2">
        <div className="flex items-baseline justify-between gap-2">
          <h2 id="seats-title" className="text-sm font-medium">
            Seats
          </h2>
          <p className="text-sm text-muted-foreground" aria-live="polite">
            {present} of {seats} here
          </p>
        </div>
        <ul aria-label="Seats" className="divide-y rounded-xl bg-card ring-1 ring-foreground/10">
          {rows.map((row) => (
            <li key={row.index} className="flex items-center justify-between gap-3 px-3 py-2.5 text-sm">
              <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                <span className="shrink-0 text-muted-foreground">Seat {row.index + 1}</span>
                <span className="max-w-full truncate font-medium">{row.name}</span>
                {row.you ? <Badge variant="secondary">you</Badge> : null}
                {row.account === null ? null : (
                  <span
                    data-slot="seat-account"
                    className="max-w-full truncate text-xs text-muted-foreground"
                  >
                    signed in as {row.account}
                  </span>
                )}
              </span>
              <span
                className={cn(
                  'flex shrink-0 items-center gap-1.5 text-xs',
                  row.here ? 'text-foreground' : 'text-muted-foreground',
                )}
              >
                {row.here ? (
                  <CircleCheck aria-hidden="true" className="size-4" />
                ) : (
                  <Circle aria-hidden="true" className="size-4" />
                )}
                {row.here ? 'Here' : 'Not here yet'}
              </span>
            </li>
          ))}
        </ul>
        {lookup.game === null && lookup.failed ? (
          <p className="text-xs text-muted-foreground">
            Could not load the names and who is here. This page keeps trying.
          </p>
        ) : null}
        {accounts.saved ? (
          <p data-slot="seat-saved" className="text-xs text-muted-foreground">
            You are signed in as {accounts.me}. This seat is saved to your account, so you can rejoin it from{' '}
            <Link to="/games" className="font-medium text-foreground underline underline-offset-3">
              My games
            </Link>{' '}
            without the link.
          </p>
        ) : null}
      </section>

      <div className="grid justify-items-start gap-2">
        <Button type="button" size="lg" disabled={ready || status !== 'open'} onClick={sayReady}>
          {ready ? 'You are ready' : 'Ready'}
        </Button>
        <p className="text-sm text-muted-foreground" aria-live="polite">
          {ready
            ? 'Waiting for the other players. You cannot take Ready back.'
            : 'Press Ready when you are set to play.'}
        </p>
        {refused ? (
          <p role="alert" className="text-sm text-destructive">
            Ready did not go through because the connection is not open. Try again in a moment.
          </p>
        ) : null}
        <p className="text-xs text-muted-foreground">
          The server only says who is connected, not who has pressed Ready, so only your own Ready is shown
          here.
        </p>
      </div>

      <Invites game={game} mine={seat} />
    </div>
  )
}

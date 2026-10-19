import { Link } from '@tanstack/react-router'
import { CircleAlertIcon } from 'lucide-react'
import { useMemo, useState } from 'react'
import type { MouseEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from './gameContext'
import { Invites } from './Invites'
import { JoinInvite } from './JoinInvite'
import { lobbyRows, summaryText } from './lobbyModel'
import { LobbySeatRow } from './LobbySeatRow'
import { LobbyVisibility } from './LobbyVisibility'
import { useLobbyAnnouncement } from './useLobbyAnnouncement'
import { useSeatAccounts } from './useSeatAccounts'

/** The seat that made the game: the only one that hands out the invite link and the code. */
const HOST_SEAT = 0

/**
 * Who is at the table before the game starts: every seat with its name, whether it is here and whether it is ready,
 * a line that sums it up, who may find the game, and this seat's Ready, which can be taken back until the game
 * starts. The host also gets the invite link and the code. The server pushes the roster at every change; before the
 * socket opens (and while it is down) the REST lookup stands in.
 */
export function Lobby() {
  const { game, session, lookup } = useGame()
  const { seat, seatCount, status, roster } = usePlay(
    useShallow((state) => ({
      seat: state.seat,
      seatCount: state.seatCount,
      status: state.status,
      roster: state.lobby,
    })),
  )
  const [refused, setRefused] = useState(false)
  const open = status === 'open'
  const rows = useMemo(
    () => lobbyRows({ roster, info: lookup.game, open, seat, seatCount }),
    [roster, lookup.game, open, seat, seatCount],
  )
  const announced = useLobbyAnnouncement(rows, roster !== null || lookup.game !== null)
  const here = rows
    .filter((row) => row.connected)
    .map((row) => row.seat)
    .join(',')
  const accounts = useSeatAccounts(game, seat, `${status}|${here}`)
  const ready = rows.find((row) => row.you)?.ready ?? false

  function toggleReady(event: MouseEvent<HTMLButtonElement>): void {
    // The second click of a double click is not a second decision: on a fast connection the label has flipped by then.
    if (event.detail > 1) return
    setRefused(!(ready ? session.notReady() : session.ready()))
  }

  // `grid-cols-1` is `minmax(0, 1fr)`: a long unbroken name or username may not stretch the column past the screen.
  return (
    <PageShell className="grid grid-cols-1 gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Waiting for players</h1>
        <p className="mt-1 text-base text-muted-foreground">
          The game starts as soon as every seat has joined and pressed Ready.
        </p>
        <LobbyVisibility visibility={lookup.game?.visibility} />
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,20rem)] lg:items-start">
        <div className="grid grid-cols-1 gap-6">
          <section aria-labelledby="seats-title" className="grid grid-cols-1 gap-2">
            <h2 id="seats-title" className="text-lg font-semibold">
              Seats
            </h2>
            <ul aria-label="Seats" className="surface divide-y">
              {rows.map((row) => (
                <LobbySeatRow key={row.seat} row={row} account={accounts.bySeat.get(row.seat) ?? null} />
              ))}
            </ul>
            <p data-slot="lobby-summary" className="text-sm [overflow-wrap:anywhere]">
              {summaryText(rows)}
            </p>
            {roster === null && lookup.game === null && lookup.failed ? (
              <p className="text-xs text-muted-foreground">
                Could not load the names and who is here. This page keeps trying.
              </p>
            ) : null}
            {accounts.saved ? (
              <p data-slot="seat-saved" className="text-xs break-words text-muted-foreground">
                You are signed in as {accounts.me}. This seat is saved to your account, so you can rejoin it
                from{' '}
                <Link to="/games" className="font-medium text-foreground underline underline-offset-3">
                  My games
                </Link>{' '}
                without the link.
              </p>
            ) : null}
            {/* No role: the connection banner is the page's status, and a second one would make both ambiguous. */}
            <p aria-live="polite" aria-atomic="true" className="sr-only" data-slot="lobby-announcement">
              {announced}
            </p>
          </section>

          <div className="grid justify-items-start gap-2">
            <Button
              type="button"
              size="lg"
              variant={ready ? 'outline' : 'default'}
              disabled={!open}
              onClick={toggleReady}
            >
              {ready ? 'Not ready' : 'Ready'}
            </Button>
            <p className="text-sm text-muted-foreground">
              {ready
                ? 'You are ready. You can take it back until the game starts.'
                : 'Press Ready when you are set to play.'}
            </p>
            {refused ? (
              <Alert variant="destructive">
                <CircleAlertIcon aria-hidden="true" />
                <AlertDescription className="text-foreground">
                  That did not go through because the connection is not open. Try again in a moment.
                </AlertDescription>
              </Alert>
            ) : null}
          </div>
        </div>

        <div className="grid grid-cols-1 gap-6 empty:hidden">
          {seat === HOST_SEAT ? <JoinInvite game={game} /> : null}
          <Invites game={game} mine={seat} />
        </div>
      </div>
    </PageShell>
  )
}

import { Link } from '@tanstack/react-router'
import { CircleAlertIcon } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import type { MouseEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { useAuth } from '@/features/auth/context'
import { loadInvites } from '@/lib/invites'
import { gameFormat } from '@/lib/houseRules'
import type { DecksApi } from '@/net/decks'
import { preconsApi as sharedPrecons } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import { DeckChooser } from './DeckChooser'
import { useGame, usePlay } from './gameContext'
import { Invites } from './Invites'
import { JoinInvite } from './JoinInvite'
import { lobbyRows, lobbyStatus, mayReady, removable, summaryText } from './lobbyModel'
import type { ReadyFacts } from './lobbyModel'
import { ReadyBar } from './ReadyBar'
import { LobbyQuestion } from './LobbyQuestion'
import { LobbyRules } from './LobbyRules'
import { LobbySeatRow } from './LobbySeatRow'
import { LobbyVisibility } from './LobbyVisibility'
import { useDeckChoice } from './useDeckChoice'
import { useLobbyAnnouncement } from './useLobbyAnnouncement'
import { useLobbyControls } from './useLobbyControls'
import type { LobbyApi } from './useLobbyControls'
import { useSeatAccounts } from './useSeatAccounts'
import { useVisibility } from './useVisibility'
import { VisibilityToggle } from './VisibilityToggle'

/** The seat that made the game: the only one that hands out the invite link and the code, lists the game, closes it and removes players. */
const HOST_SEAT = 0

/** What `Lobby` takes. */
export interface LobbyProps {
  /** The calls of the controls: the shared client in the app. */
  api: LobbyApi
  /** The signed-in account's decks, for the deck chooser: the shared client in the app. */
  decks: DecksApi
  /** The precons for the deck chooser: the shared client in the app. */
  precons?: Pick<PreconsApi, 'list' | 'get'>
  /** This seat's token, or undefined when the page acts through the signed-in session. */
  token: string | undefined
}

/**
 * Who is at the table before the game starts: every seat with its name, its deck (name, format and how many cards
 * need manual steps), whether it is here and whether it is ready, a line that sums it up, who may find the game, this
 * seat's deck chooser until it is ready, and this seat's Ready, which needs a deck and can be taken back until the game
 * starts. The host also gets the invite link and the code, a toggle for who may find the game, a Remove button on
 * every player who joined and Close game; everyone else gets Leave game. Each of those that cannot be undone asks
 * first. The server pushes the roster at every change; before the socket opens (and while it is down) the REST
 * lookup stands in.
 */
export function Lobby({ api, decks, token, precons = sharedPrecons }: LobbyProps) {
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
  const [previewing, setPreviewing] = useState<ReadyFacts['previewing']>(null)
  const open = status === 'open'
  const host = seat === HOST_SEAT
  const known = lookup.game?.visibility
  const tableFormat = lookup.game?.format ?? 'normal'
  const tableRules = lookup.game?.house_rules
  const controls = useLobbyControls({ api, game, token, started: lookup.game?.started === true })
  const visibility = useVisibility({
    api,
    game,
    credential: controls.credential,
    known,
    run: controls.run,
    stop: controls.stop,
  })
  // The seats this tab holds a token for came with the game; the server cannot give those back.
  const [issued] = useState<ReadonlySet<number>>(
    () => new Set(loadInvites(game)?.seats.map((invite) => invite.seat) ?? []),
  )
  const seatsHeading = useRef<HTMLHeadingElement>(null)
  const note = useRef<HTMLDivElement>(null)
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
  const mine = rows.find((row) => row.you)
  const ready = mine?.ready ?? false
  // A seat the roster shows without a deck cannot press Ready: the server would refuse it, so the button says why.
  const needsDeck = !ready && mine !== undefined && !mayReady(mine)
  const editable = controls.stopped === null
  const signedIn = useAuth((state) => state.status === 'signed-in')
  const deckChoice = useDeckChoice({
    api,
    game,
    seat: seat ?? 0,
    credential: controls.credential,
    current: mine?.deck,
    run: controls.run,
    stop: controls.stop,
  })

  // A request that found the game started has taken the buttons away: the words that say so take the focus.
  const { stoppedByRequest, asking } = controls
  useEffect(() => {
    if (stoppedByRequest && asking === null) note.current?.focus()
  }, [stoppedByRequest, asking])

  function toggleReady(event: MouseEvent<HTMLButtonElement>): void {
    // The second click of a double click is not a second decision: on a fast connection the label has flipped by then.
    if (event.detail > 1) return
    setRefused(!(ready ? session.notReady() : session.ready()))
  }

  const summary = lobbyStatus(rows)
  const format = gameFormat(tableFormat)

  // `grid-cols-1` is `minmax(0, 1fr)`: a long unbroken name or username may not stretch the column past the screen.
  return (
    <PageShell
      width="wide"
      className="grid grid-cols-1 gap-6 min-[1000px]:grid-cols-[minmax(0,22rem)_minmax(0,1fr)] min-[1280px]:grid-cols-[minmax(0,26rem)_minmax(0,1fr)] min-[1000px]:grid-rows-[auto_1fr] min-[1000px]:items-start"
    >
      <div
        data-slot="lobby-game"
        className="grid min-w-0 grid-cols-1 gap-4 min-[1000px]:col-start-1 min-[1000px]:row-start-1"
      >
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Waiting for players</h1>
          <p data-slot="lobby-format" className="mt-1 text-base">
            <span className="font-medium">{format === 'commander' ? 'Commander' : 'Normal'} game</span>
            <span className="text-muted-foreground"> · {rows.length || seatCount || '?'} seats</span>
          </p>
          <p className="mt-1 text-sm text-muted-foreground">
            The game starts as soon as every seat has joined and pressed Ready.
          </p>
          <LobbyRules rules={tableRules} format={tableFormat} />
          {host && editable && visibility.shown !== undefined ? (
            <VisibilityToggle shown={visibility.shown} control={visibility} busy={controls.busy} />
          ) : (
            <LobbyVisibility visibility={known} />
          )}
        </div>

        <section
          aria-labelledby="seats-title"
          data-slot="lobby-status"
          data-done={summary.done || undefined}
          className="surface grid gap-3 p-4"
        >
          <div className="flex items-baseline justify-between gap-3">
            <h2
              id="seats-title"
              ref={seatsHeading}
              tabIndex={-1}
              className="text-lg font-semibold outline-none"
            >
              Seats
            </h2>
            <span data-slot="lobby-progress" className="text-sm text-muted-foreground tabular-nums">
              {summary.ready} of {summary.seats} ready
            </span>
          </div>
          <div
            aria-hidden="true"
            className="h-1.5 overflow-hidden rounded-full bg-muted"
            data-slot="lobby-progress-bar"
          >
            <span
              className="block h-full rounded-full bg-support-verified transition-[width] duration-base ease-out"
              style={{ width: `${summary.seats === 0 ? 0 : (summary.ready / summary.seats) * 100}%` }}
            />
          </div>
          <p data-slot="lobby-headline" className="text-base font-medium">
            {summary.headline}
          </p>
          <ul aria-label="Seats" className="divide-y rounded-lg border">
            {rows.map((row) => (
              <LobbySeatRow
                key={row.seat}
                row={row}
                host={row.seat === HOST_SEAT}
                account={accounts.bySeat.get(row.seat) ?? null}
                remove={
                  host && editable && removable(row, HOST_SEAT, issued)
                    ? {
                        disabled: controls.busy,
                        onPress: (opener) =>
                          controls.ask({ kind: 'remove', seat: row.seat, name: row.name }, opener),
                      }
                    : undefined
                }
              />
            ))}
          </ul>
          <p data-slot="lobby-summary" className="text-sm text-muted-foreground [overflow-wrap:anywhere]">
            {summaryText(rows)}
          </p>
          <p
            data-slot="lobby-removed"
            aria-live="polite"
            aria-atomic="true"
            className="text-sm [overflow-wrap:anywhere] empty:hidden"
          >
            {controls.removed}
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

        {host ? <JoinInvite game={game} /> : null}
        <Invites game={game} mine={seat} />
      </div>

      {/* On a phone this wrapper takes no box of its own, so the Ready bar can stick to the bottom of the page. */}
      <div
        data-slot="lobby-deck"
        className="contents min-[1000px]:col-start-2 min-[1000px]:row-span-2 min-[1000px]:row-start-1 min-[1000px]:sticky min-[1000px]:top-6 min-[1000px]:flex min-[1000px]:h-[calc(100dvh-7.5rem)] min-[1000px]:min-h-136 min-[1000px]:flex-col min-[1000px]:gap-3"
      >
        {seat === null || !editable ? null : (
          <DeckChooser
            api={api}
            decks={decks}
            precons={precons}
            game={game}
            format={format}
            signedIn={signedIn}
            current={mine?.deck}
            ready={ready}
            busy={controls.busy}
            choice={deckChoice}
            onPreview={setPreviewing}
          />
        )}
        <ReadyBar
          open={open}
          busy={controls.busy}
          ready={ready}
          needsDeck={needsDeck}
          previewing={previewing}
          onToggle={toggleReady}
        >
          {refused ? (
            <Alert variant="destructive">
              <CircleAlertIcon aria-hidden="true" />
              <AlertDescription className="text-foreground">
                That did not go through because the connection is not open. Try again in a moment.
              </AlertDescription>
            </Alert>
          ) : null}
          {controls.stopped === null ? null : (
            <Alert ref={note} tabIndex={-1} data-slot="lobby-stopped" className="outline-none">
              <CircleAlertIcon aria-hidden="true" />
              <AlertDescription className="text-foreground">{controls.stopped}</AlertDescription>
            </Alert>
          )}
        </ReadyBar>
      </div>

      {/* After Ready in the reading order, so the one control that ends something is the last. */}
      {!editable ? null : (
        <div
          data-slot="lobby-leave"
          className="flex flex-wrap items-center gap-3 border-t pt-4 min-[1000px]:col-start-1 min-[1000px]:row-start-2"
        >
          {host ? (
            <Button
              type="button"
              variant="destructive"
              data-slot="close-game"
              disabled={controls.busy}
              onClick={(event) => controls.ask({ kind: 'close' }, event.currentTarget)}
            >
              Close game
            </Button>
          ) : (
            <Button
              type="button"
              variant="outline"
              data-slot="leave-game"
              disabled={controls.busy}
              onClick={(event) => controls.ask({ kind: 'leave' }, event.currentTarget)}
            >
              Leave game
            </Button>
          )}
          <span className="text-sm text-muted-foreground">
            {host ? 'Ends the game for everyone.' : 'Gives your seat back.'}
          </span>
        </div>
      )}

      {controls.asking === null ? null : (
        <LobbyQuestion
          asking={controls.asking}
          busy={controls.busy}
          seats={() => seatsHeading.current}
          note={() => note.current}
          onConfirm={controls.confirm}
          onCancel={controls.cancel}
          onClosed={controls.closed}
        />
      )}
    </PageShell>
  )
}

import { Link, useNavigate } from '@tanstack/react-router'
import { CircleAlertIcon, RefreshCwIcon } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Skeleton } from '@/components/ui/skeleton'
import { usePageTitle } from '@/hooks/usePageTitle'
import { TournamentError, tournamentsApi } from '@/net/tournaments'
import type { TournamentsApi } from '@/net/tournaments'
import type { MatchOutcome, MatchView, RoundView, TournamentView } from '@/protocol/generated'
import {
  entrantName,
  formatLabel,
  inviteLink,
  isMine,
  outcomeText,
  percent,
  roundDecided,
  statusLabel,
  structureLabel,
} from './format'
import { SignedInOnly } from './SignedInOnly'

/** How often a running tournament is read again, so finished games show their results, in milliseconds. */
export const REFRESH_MS = 15_000

function errorText(error: unknown): string {
  return error instanceof TournamentError ? error.message : 'Something went wrong. Try again.'
}

/** One action under way and the result of the last. */
interface Actions {
  busy: string | null
  error: string | null
  run: (label: string, action: () => Promise<TournamentView | void>) => void
}

function OrganiserPanel({
  tournament,
  actions,
  client,
}: {
  tournament: TournamentView
  actions: Actions
  client: TournamentsApi
}) {
  const navigate = useNavigate()
  const link = tournament.invite === null ? null : inviteLink(window.location.origin, tournament.invite)
  const decided = roundDecided(tournament)
  const missingGames = tournament.rounds_played
    .find((r) => r.round === tournament.round)
    ?.matches.some((m) => m.b !== null && m.game === null && m.outcome === null)
  const lastRound =
    tournament.structure === 'swiss' && tournament.rounds !== null && tournament.round >= tournament.rounds
  return (
    <section aria-labelledby="organiser-heading" className="surface grid gap-3 p-4">
      <h2 id="organiser-heading" className="text-lg font-semibold">
        Organiser
      </h2>
      {link !== null && tournament.status === 'signup' ? (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm text-muted-foreground">Invite link</span>
          <code className="min-w-0 truncate rounded-md bg-muted px-2 py-1 text-sm">{link}</code>
          <CopyButton text={link} label="Copy the invite link" />
        </div>
      ) : null}
      {!tournament.playable ? (
        <p className="text-sm text-muted-foreground">
          Team games are not available on this server yet, so this tournament cannot start.
        </p>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {tournament.status === 'signup' ? (
          <Button
            type="button"
            loading={actions.busy === 'start'}
            disabled={!tournament.playable || tournament.entrants.length < 2}
            onClick={() => actions.run('start', () => client.act(tournament.id, 'start'))}
          >
            Start round 1
          </Button>
        ) : null}
        {tournament.status === 'running' ? (
          <Button
            type="button"
            loading={actions.busy === 'next'}
            aria-disabled={!decided || undefined}
            onClick={() => {
              if (decided) actions.run('next', () => client.act(tournament.id, 'next'))
            }}
          >
            {lastRound ? 'Finish tournament' : 'Pair next round'}
          </Button>
        ) : null}
        {tournament.status === 'running' && missingGames === true ? (
          <Button
            type="button"
            variant="outline"
            loading={actions.busy === 'games'}
            onClick={() => actions.run('games', () => client.act(tournament.id, 'games'))}
          >
            Make missing games
          </Button>
        ) : null}
        <Button
          type="button"
          variant="destructive"
          loading={actions.busy === 'remove'}
          onClick={() => {
            if (!window.confirm(`Delete ${tournament.name}? Its games go on as normal games.`)) return
            actions.run('remove', async () => {
              await client.remove(tournament.id)
              await navigate({ to: '/tournaments' })
            })
          }}
        >
          Delete tournament
        </Button>
      </div>
      {tournament.status === 'running' && !decided ? (
        <p className="text-sm text-muted-foreground">
          The next round is paired once every match has a result.
        </p>
      ) : null}
    </section>
  )
}

function Entrants({
  tournament,
  actions,
  client,
}: {
  tournament: TournamentView
  actions: Actions
  client: TournamentsApi
}) {
  const mine = tournament.entrants.find((e) => e.mine)
  return (
    <section aria-labelledby="entrants-heading" className="grid gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 id="entrants-heading" className="text-lg font-semibold">
          Entrants ({tournament.entrants.length})
        </h2>
        {mine !== undefined && tournament.status === 'signup' ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            loading={actions.busy === 'leave'}
            onClick={() => actions.run('leave', () => client.leave(tournament.id))}
          >
            Leave
          </Button>
        ) : null}
        {mine !== undefined && !mine.dropped && tournament.status === 'running' ? (
          <Button
            type="button"
            variant="destructive"
            size="sm"
            loading={actions.busy === 'drop'}
            onClick={() => {
              if (window.confirm('Drop out? You will not be paired again.')) {
                actions.run('drop', () => client.act(tournament.id, 'drop'))
              }
            }}
          >
            Drop out
          </Button>
        ) : null}
      </div>
      {tournament.entrants.length === 0 ? (
        <p className="text-sm text-muted-foreground">Nobody has signed up yet.</p>
      ) : (
        <ul className="surface">
          {tournament.entrants.map((entrant) => (
            <li
              key={entrant.entrant}
              className="flex flex-wrap items-center justify-between gap-2 border-b px-4 py-2 last:border-b-0"
            >
              <span>
                <span className="font-medium">{entrant.name}</span>
                {entrant.players.length > 1 || entrant.players[0] !== entrant.name ? (
                  <span className="text-sm text-muted-foreground"> · {entrant.players.join(', ')}</span>
                ) : null}
              </span>
              <span className="flex items-center gap-2">
                {entrant.mine ? <Pill tone="success">You</Pill> : null}
                {entrant.dropped ? <Pill tone="warning">Dropped</Pill> : null}
                {tournament.organiser && tournament.status === 'running' && !entrant.dropped ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="xs"
                    aria-label={`Drop ${entrant.name}`}
                    loading={actions.busy === `drop-${entrant.entrant}`}
                    onClick={() => {
                      if (window.confirm(`Drop ${entrant.name}? They will not be paired again.`)) {
                        actions.run(`drop-${entrant.entrant}`, () =>
                          client.dropEntrant(tournament.id, entrant.entrant),
                        )
                      }
                    }}
                  >
                    Drop
                  </Button>
                ) : null}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

/** The organiser's result picker: the game's result, or a correction. */
function OutcomePicker({
  tournament,
  round,
  match,
  actions,
  client,
}: {
  tournament: TournamentView
  round: number
  match: MatchView
  actions: Actions
  client: TournamentsApi
}) {
  if (match.b === null) return null
  const a = entrantName(tournament.entrants, match.a)
  const b = entrantName(tournament.entrants, match.b)
  return (
    <NativeSelect
      size="sm"
      aria-label={`Result of table ${match.table}`}
      value={match.outcome ?? ''}
      disabled={actions.busy !== null}
      onChange={(event) => {
        const outcome = event.target.value as MatchOutcome
        actions.run(`result-${round}-${match.table}`, () =>
          client.setOutcome(tournament.id, round, match.table, outcome),
        )
      }}
    >
      {match.outcome === null ? <NativeSelectOption value="">No result</NativeSelectOption> : null}
      <NativeSelectOption value="a">{a} won</NativeSelectOption>
      <NativeSelectOption value="b">{b} won</NativeSelectOption>
      <NativeSelectOption value="draw">Draw</NativeSelectOption>
    </NativeSelect>
  )
}

function Round({
  tournament,
  round,
  actions,
  client,
}: {
  tournament: TournamentView
  round: RoundView
  actions: Actions
  client: TournamentsApi
}) {
  const current = round.round === tournament.round && tournament.status === 'running'
  const editable =
    tournament.organiser && tournament.status === 'running' && (tournament.structure === 'swiss' || current)
  return (
    <section aria-label={`Round ${round.round}`} className="grid gap-2">
      <h3 className="font-semibold">Round {round.round}</h3>
      <ul className="surface">
        {round.matches.map((match) => {
          const mine = isMine(tournament.entrants, match)
          return (
            <li
              key={match.table}
              className="flex flex-wrap items-center justify-between gap-2 border-b px-4 py-2 last:border-b-0"
            >
              <span className="grid gap-0.5">
                <span>
                  <span className="text-sm text-muted-foreground">Table {match.table} · </span>
                  {match.b === null
                    ? entrantName(tournament.entrants, match.a)
                    : `${entrantName(tournament.entrants, match.a)} vs ${entrantName(tournament.entrants, match.b)}`}
                </span>
                <span className="text-sm text-muted-foreground">
                  {outcomeText(tournament.entrants, match)}
                  {match.corrected ? ' (set by the organiser)' : ''}
                </span>
              </span>
              <span className="flex items-center gap-2">
                {mine && current && match.game !== null && match.outcome === null ? (
                  <Link
                    to="/game/$gameId"
                    params={{ gameId: match.game }}
                    className={buttonVariants({ size: 'sm' })}
                  >
                    Play your game
                  </Link>
                ) : null}
                {match.b !== null && match.game === null && match.outcome === null ? (
                  <Pill tone="warning">No game yet</Pill>
                ) : null}
                {editable ? (
                  <OutcomePicker
                    tournament={tournament}
                    round={round.round}
                    match={match}
                    actions={actions}
                    client={client}
                  />
                ) : null}
              </span>
            </li>
          )
        })}
      </ul>
    </section>
  )
}

function Standings({ tournament }: { tournament: TournamentView }) {
  if (tournament.standings.length === 0) return null
  return (
    <section aria-labelledby="standings-heading" className="grid gap-2">
      <h2 id="standings-heading" className="text-lg font-semibold">
        {tournament.status === 'finished' ? 'Final standings' : 'Standings'}
      </h2>
      <div className="surface overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="text-left text-muted-foreground">
            <tr className="border-b">
              <th scope="col" className="px-3 py-2 font-medium">
                #
              </th>
              <th scope="col" className="px-3 py-2 font-medium">
                Entrant
              </th>
              <th scope="col" className="px-3 py-2 text-right font-medium">
                Points
              </th>
              <th scope="col" className="px-3 py-2 text-right font-medium">
                W-L-D
              </th>
              <th scope="col" className="px-3 py-2 text-right font-medium">
                <abbr title="Opponents' match-win percentage">OMW%</abbr>
              </th>
              <th scope="col" className="px-3 py-2 text-right font-medium">
                <abbr title="Game-win percentage">GW%</abbr>
              </th>
              <th scope="col" className="px-3 py-2 text-right font-medium">
                <abbr title="Opponents' game-win percentage">OGW%</abbr>
              </th>
            </tr>
          </thead>
          <tbody className="tabular-nums">
            {tournament.standings.map((row) => (
              <tr key={row.entrant} className="border-b last:border-b-0">
                <td className="px-3 py-2">{row.rank}</td>
                <td className="px-3 py-2">{entrantName(tournament.entrants, row.entrant)}</td>
                <td className="px-3 py-2 text-right font-medium">{row.points}</td>
                <td className="px-3 py-2 text-right">
                  {row.wins}-{row.losses}-{row.draws}
                </td>
                <td className="px-3 py-2 text-right">{percent(row.opponents_match_win)}</td>
                <td className="px-3 py-2 text-right">{percent(row.game_win)}</td>
                <td className="px-3 py-2 text-right">{percent(row.opponents_game_win)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

/** The tournament once loaded: facts, the organiser's controls, entrants, rounds and standings. */
export function TournamentDetail({
  tournament,
  actions,
  client,
  onRefresh,
}: {
  tournament: TournamentView
  actions: Actions
  client: TournamentsApi
  onRefresh: () => void
}) {
  const rounds = [...tournament.rounds_played].reverse()
  return (
    <>
      <header className="grid gap-1">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h1 className="text-2xl font-semibold tracking-tight">{tournament.name}</h1>
          <Button type="button" variant="outline" size="sm" onClick={onRefresh}>
            <RefreshCwIcon aria-hidden="true" />
            Refresh
          </Button>
        </div>
        <p className="text-sm text-muted-foreground">
          {formatLabel(tournament.format)} · {structureLabel(tournament.structure)} · organised by{' '}
          {tournament.organiser_name} ·{' '}
          <span role="status">{statusLabel(tournament.status, tournament.round, tournament.rounds)}</span>
        </p>
      </header>
      {actions.error === null ? null : (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>That did not work</AlertTitle>
          <AlertDescription>{actions.error}</AlertDescription>
        </Alert>
      )}
      {tournament.organiser ? (
        <OrganiserPanel tournament={tournament} actions={actions} client={client} />
      ) : null}
      <Standings tournament={tournament} />
      {rounds.length > 0 ? (
        <section aria-labelledby="rounds-heading" className="grid gap-4">
          <h2 id="rounds-heading" className="text-lg font-semibold">
            Pairings
          </h2>
          {rounds.map((round) => (
            <Round
              key={round.round}
              tournament={tournament}
              round={round}
              actions={actions}
              client={client}
            />
          ))}
        </section>
      ) : null}
      <Entrants tournament={tournament} actions={actions} client={client} />
    </>
  )
}

function Loaded({ id, client }: { id: string; client: TournamentsApi }) {
  const [tournament, setTournament] = useState<TournamentView | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  usePageTitle(tournament?.name ?? 'Tournament')

  const load = useCallback(() => {
    client
      .get(id)
      .then((view) => {
        setTournament(view)
        setLoadError(null)
      })
      .catch((caught: unknown) => setLoadError(errorText(caught)))
  }, [client, id])

  useEffect(() => load(), [load])
  const running = tournament?.status === 'running'
  useEffect(() => {
    if (!running) return
    const timer = window.setInterval(load, REFRESH_MS)
    return () => window.clearInterval(timer)
  }, [running, load])

  const actions: Actions = {
    busy,
    error,
    run: (label, action) => {
      setBusy(label)
      setError(null)
      action()
        .then((view) => {
          setBusy(null)
          if (view) setTournament(view)
          else load()
        })
        .catch((caught: unknown) => {
          setBusy(null)
          setError(errorText(caught))
        })
    },
  }

  if (tournament === null) {
    if (loadError !== null) {
      return (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertTitle>Could not load the tournament</AlertTitle>
          <AlertDescription>{loadError}</AlertDescription>
        </Alert>
      )
    }
    return (
      <Loading label="Loading the tournament…">
        <Skeleton className="h-40" />
      </Loading>
    )
  }
  return <TournamentDetail tournament={tournament} actions={actions} client={client} onRefresh={load} />
}

/** `/tournaments/$tournamentId`: one tournament for its organiser and players. */
export function TournamentPage({ id, client = tournamentsApi }: { id: string; client?: TournamentsApi }) {
  return (
    <PageShell width="wide" className="grid gap-6 sm:py-6">
      <SignedInOnly next={`/tournaments/${id}`}>
        <Loaded id={id} client={client} />
      </SignedInOnly>
    </PageShell>
  )
}

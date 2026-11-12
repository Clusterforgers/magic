import { Link, useNavigate } from '@tanstack/react-router'
import { CircleAlertIcon, TrophyIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Skeleton } from '@/components/ui/skeleton'
import { usePageTitle } from '@/hooks/usePageTitle'
import { TournamentError, tournamentsApi } from '@/net/tournaments'
import type { TournamentsApi } from '@/net/tournaments'
import type { TournamentFormat, TournamentStructure, TournamentSummary } from '@/protocol/generated'
import { formatLabel, statusLabel, structureLabel } from './format'
import { SignedInOnly } from './SignedInOnly'

/** Longest tournament name, as the server takes it. */
export const MAX_NAME_CHARS = 60

function errorText(error: unknown): string {
  return error instanceof TournamentError ? error.message : 'Something went wrong. Try again.'
}

/** The organiser's form: name, format, structure and, for Swiss, the number of rounds. */
export function CreateTournamentForm({ client }: { client: TournamentsApi }) {
  const navigate = useNavigate()
  const [name, setName] = useState('')
  const [format, setFormat] = useState<TournamentFormat>('commander_1v1')
  const [structure, setStructure] = useState<TournamentStructure>('swiss')
  const [rounds, setRounds] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = (event: FormEvent): void => {
    event.preventDefault()
    if (name.trim() === '') {
      setError('Give the tournament a name.')
      return
    }
    setBusy(true)
    setError(null)
    const planned = structure === 'swiss' && rounds !== '' ? Number(rounds) : undefined
    client
      .create({ name: name.trim(), format, structure, rounds: planned })
      .then((created) => navigate({ to: '/tournaments/$tournamentId', params: { tournamentId: created.id } }))
      .catch((caught: unknown) => {
        setBusy(false)
        setError(errorText(caught))
      })
  }

  return (
    <form onSubmit={submit} aria-label="New tournament" className="surface grid gap-4 p-4" noValidate>
      <h2 className="text-lg font-semibold">New tournament</h2>
      <div className="grid gap-1">
        <Label htmlFor="tournament-name">Name</Label>
        <Input
          id="tournament-name"
          value={name}
          maxLength={MAX_NAME_CHARS}
          aria-invalid={error !== null && name.trim() === ''}
          onChange={(event) => setName(event.target.value)}
        />
      </div>
      <div className="flex flex-wrap gap-4">
        <div className="grid gap-1">
          <Label htmlFor="tournament-format">Format</Label>
          <NativeSelect
            id="tournament-format"
            value={format}
            onChange={(event) => setFormat(event.target.value as TournamentFormat)}
          >
            <NativeSelectOption value="commander_1v1">{formatLabel('commander_1v1')}</NativeSelectOption>
            <NativeSelectOption value="commander_2v2">{formatLabel('commander_2v2')}</NativeSelectOption>
          </NativeSelect>
        </div>
        <div className="grid gap-1">
          <Label htmlFor="tournament-structure">Rounds are paired by</Label>
          <NativeSelect
            id="tournament-structure"
            value={structure}
            onChange={(event) => setStructure(event.target.value as TournamentStructure)}
          >
            <NativeSelectOption value="swiss">{structureLabel('swiss')}</NativeSelectOption>
            <NativeSelectOption value="elimination">{structureLabel('elimination')}</NativeSelectOption>
          </NativeSelect>
        </div>
        {structure === 'swiss' ? (
          <div className="grid gap-1">
            <Label htmlFor="tournament-rounds">Rounds</Label>
            <NativeSelect
              id="tournament-rounds"
              value={rounds}
              onChange={(event) => setRounds(event.target.value)}
            >
              <NativeSelectOption value="">Enough for one winner</NativeSelectOption>
              {Array.from({ length: 15 }, (_, i) => i + 1).map((n) => (
                <NativeSelectOption key={n} value={String(n)}>
                  {n}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </div>
        ) : null}
      </div>
      {format === 'commander_2v2' ? (
        <p className="text-sm text-muted-foreground">
          Players sign up as teams of two. Team games come with Team Commander; until then a 2v2 tournament
          can take sign-ups but not start.
        </p>
      ) : null}
      {error === null ? null : (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div>
        <Button type="submit" loading={busy}>
          Create tournament
        </Button>
      </div>
    </form>
  )
}

function TournamentRow({ tournament }: { tournament: TournamentSummary }) {
  return (
    <li className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 border-b px-4 py-3 last:border-b-0">
      <div className="grid gap-0.5">
        <Link
          to="/tournaments/$tournamentId"
          params={{ tournamentId: tournament.id }}
          className="font-medium underline-offset-4 hover:underline"
        >
          {tournament.name}
        </Link>
        <span className="text-sm text-muted-foreground">
          {formatLabel(tournament.format)} · {structureLabel(tournament.structure)} · {tournament.entrants}{' '}
          {tournament.entrants === 1 ? 'entrant' : 'entrants'}
        </span>
      </div>
      <div className="flex items-center gap-2">
        {tournament.organiser ? <Pill>Organiser</Pill> : null}
        <Pill tone={tournament.status === 'running' ? 'success' : 'neutral'}>
          {statusLabel(tournament.status, tournament.round, null)}
        </Pill>
      </div>
    </li>
  )
}

/** The player's tournaments, newest first. */
function TournamentListPanel({ client }: { client: TournamentsApi }) {
  const [list, setList] = useState<TournamentSummary[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    client
      .list()
      .then((answer) => live && setList(answer.tournaments))
      .catch((caught: unknown) => live && setError(errorText(caught)))
    return () => {
      live = false
    }
  }, [client])
  if (error !== null) {
    return (
      <Alert variant="destructive">
        <CircleAlertIcon aria-hidden="true" />
        <AlertTitle>Could not load your tournaments</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }
  if (list === null) {
    return (
      <Loading label="Loading your tournaments…">
        <Skeleton className="h-24" />
      </Loading>
    )
  }
  if (list.length === 0) {
    return (
      <EmptyState icon={TrophyIcon} title="No tournaments yet">
        Tournaments you organise, and those you sign up for with an invite link, are listed here.
      </EmptyState>
    )
  }
  return (
    <ul aria-label="Your tournaments" className="surface">
      {list.map((tournament) => (
        <TournamentRow key={tournament.id} tournament={tournament} />
      ))}
    </ul>
  )
}

/** `/tournaments`: the player's tournaments and the form for a new one. */
export function TournamentsPage({ client = tournamentsApi }: { client?: TournamentsApi }) {
  usePageTitle('Tournaments')
  return (
    <PageShell width="page" className="grid gap-6 sm:py-6">
      <h1 className="text-2xl font-semibold tracking-tight">Tournaments</h1>
      <SignedInOnly next="/tournaments">
        <TournamentListPanel client={client} />
        <CreateTournamentForm client={client} />
      </SignedInOnly>
    </PageShell>
  )
}

import { Link, useNavigate } from '@tanstack/react-router'
import { CircleAlertIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { usePageTitle } from '@/hooks/usePageTitle'
import { TournamentError, tournamentsApi } from '@/net/tournaments'
import type { TournamentsApi } from '@/net/tournaments'
import type { TournamentInvite } from '@/protocol/generated'
import { formatLabel, openTeams, structureLabel } from './format'
import { MAX_NAME_CHARS } from './TournamentsPage'
import { SignedInOnly } from './SignedInOnly'

function errorText(error: unknown): string {
  return error instanceof TournamentError ? error.message : 'Something went wrong. Try again.'
}

/** The sign-up form: alone, or in a team format a new team or a place in one. */
export function SignUpForm({
  code,
  invite,
  client,
}: {
  code: string
  invite: TournamentInvite
  client: TournamentsApi
}) {
  const navigate = useNavigate()
  const teams = invite.format === 'commander_2v2'
  const open = openTeams(invite.entrants, invite.format)
  // '' starts a new team; a number takes the open place of that entrant.
  const [team, setTeam] = useState('')
  const [teamName, setTeamName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = (event: FormEvent): void => {
    event.preventDefault()
    setBusy(true)
    setError(null)
    const body =
      !teams || team !== ''
        ? { team: teams && team !== '' ? Number(team) : undefined }
        : { team_name: teamName.trim() === '' ? undefined : teamName.trim() }
    client
      .join(code, body)
      .then((view) => navigate({ to: '/tournaments/$tournamentId', params: { tournamentId: view.id } }))
      .catch((caught: unknown) => {
        setBusy(false)
        setError(errorText(caught))
      })
  }

  return (
    <form onSubmit={submit} aria-label="Sign up" className="grid gap-4">
      {teams ? (
        <fieldset className="grid gap-2">
          <legend className="text-sm font-medium">Your team</legend>
          <label className="flex items-center gap-2 text-sm">
            <input type="radio" name="team" value="" checked={team === ''} onChange={() => setTeam('')} />
            Start a new team
          </label>
          {team === '' ? (
            <div className="grid gap-1 pl-6">
              <Label htmlFor="team-name">Team name (optional)</Label>
              <Input
                id="team-name"
                value={teamName}
                maxLength={MAX_NAME_CHARS}
                onChange={(event) => setTeamName(event.target.value)}
              />
            </div>
          ) : null}
          {open.map((entrant) => (
            <label key={entrant.entrant} className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="team"
                value={String(entrant.entrant)}
                checked={team === String(entrant.entrant)}
                onChange={() => setTeam(String(entrant.entrant))}
              />
              Join {entrant.name} ({entrant.players.join(', ')})
            </label>
          ))}
        </fieldset>
      ) : null}
      {error === null ? null : (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <div>
        <Button type="submit" loading={busy}>
          Sign up
        </Button>
      </div>
    </form>
  )
}

function Invite({ code, client }: { code: string; client: TournamentsApi }) {
  const [invite, setInvite] = useState<TournamentInvite | null>(null)
  const [error, setError] = useState<string | null>(null)
  usePageTitle(invite === null ? 'Tournament invite' : `Join ${invite.name}`)
  useEffect(() => {
    let live = true
    client
      .invite(code)
      .then((answer) => live && setInvite(answer))
      .catch((caught: unknown) => {
        if (!live) return
        const missing = caught instanceof TournamentError && caught.code === 'not_found'
        setError(
          missing ? 'This invite opens no tournament. Check the link with the organiser.' : errorText(caught),
        )
      })
    return () => {
      live = false
    }
  }, [client, code])

  if (error !== null) {
    return (
      <Alert variant="destructive">
        <CircleAlertIcon aria-hidden="true" />
        <AlertTitle>Could not open the invite</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    )
  }
  if (invite === null) {
    return (
      <Loading label="Opening the invite…">
        <Skeleton className="h-32" />
      </Loading>
    )
  }
  const goTo = (
    <Link to="/tournaments/$tournamentId" params={{ tournamentId: invite.id }} className={buttonVariants()}>
      Open the tournament
    </Link>
  )
  return (
    <section className="surface grid gap-4 p-4">
      <header className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">{invite.name}</h1>
        <p className="text-sm text-muted-foreground">
          {formatLabel(invite.format)} · {structureLabel(invite.structure)} · organised by{' '}
          {invite.organiser_name} · {invite.entrants.length}{' '}
          {invite.entrants.length === 1 ? 'entrant' : 'entrants'} so far
        </p>
      </header>
      {invite.joined ? (
        <>
          <p role="status">You have signed up.</p>
          <div>{goTo}</div>
        </>
      ) : invite.status === 'signup' ? (
        <SignUpForm code={code} invite={invite} client={client} />
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          This tournament has started, so sign-up is closed.
        </p>
      )}
    </section>
  )
}

/** `/tournaments/join/$code`: what an invite signs up for, and the sign-up. */
export function JoinTournamentPage({
  code,
  client = tournamentsApi,
}: {
  code: string
  client?: TournamentsApi
}) {
  return (
    <PageShell width="narrow" className="grid gap-6 sm:py-6">
      <SignedInOnly next={`/tournaments/join/${encodeURIComponent(code)}`}>
        <Invite code={code} client={client} />
      </SignedInOnly>
    </PageShell>
  )
}

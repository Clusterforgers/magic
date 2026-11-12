import { useParams } from '@tanstack/react-router'
import { JoinTournamentPage } from './JoinTournamentPage'
import { TournamentPage } from './TournamentPage'

/** `/tournaments/$tournamentId`, keyed by the id so another tournament starts fresh. */
export function TournamentRoute() {
  const { tournamentId } = useParams({ from: '/tournaments_/$tournamentId' })
  return <TournamentPage key={tournamentId} id={tournamentId} />
}

/** `/tournaments/join/$code`, keyed by the code. */
export function JoinTournamentRoute() {
  const { code } = useParams({ from: '/tournaments_/join/$code' })
  return <JoinTournamentPage key={code} code={code} />
}

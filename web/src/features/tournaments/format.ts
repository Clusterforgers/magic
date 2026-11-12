/** Words for tournaments: formats, structures, statuses, results and the tiebreaker fractions. */
import type {
  EntrantView,
  MatchOutcome,
  MatchView,
  TournamentFormat,
  TournamentStatus,
  TournamentStructure,
  TournamentView,
} from '@/protocol/generated'

/** The format as the pages name it. */
export function formatLabel(format: TournamentFormat): string {
  return format === 'commander_1v1' ? 'Commander 1v1' : 'Commander 2v2'
}

/** The structure as the pages name it. */
export function structureLabel(structure: TournamentStructure): string {
  return structure === 'swiss' ? 'Swiss' : 'Single elimination'
}

/** Where the tournament is, in words: "Sign-up", "Round 2 of 4", "Finished". */
export function statusLabel(status: TournamentStatus, round: number, rounds: number | null): string {
  if (status === 'signup') return 'Sign-up'
  if (status === 'finished') return 'Finished'
  return rounds === null ? `Round ${round}` : `Round ${round} of ${rounds}`
}

/** A fraction as a percentage with one decimal: 0.3333 is "33.3%". */
export function percent(fraction: number): string {
  return `${(fraction * 100).toFixed(1)}%`
}

/** An entrant's name by number; a number nobody has is shown as such. */
export function entrantName(entrants: readonly EntrantView[], entrant: number): string {
  return entrants.find((e) => e.entrant === entrant)?.name ?? `Entrant ${entrant + 1}`
}

/** A match's result in words, from the names of its entrants. */
export function outcomeText(entrants: readonly EntrantView[], match: MatchView): string {
  if (match.b === null) return `${entrantName(entrants, match.a)} has a bye`
  const outcome: MatchOutcome | null = match.outcome
  if (outcome === null) return 'Playing'
  if (outcome === 'draw') return 'Draw'
  return `${entrantName(entrants, outcome === 'a' ? match.a : match.b)} won`
}

/** Whether the signed-in player plays in the match. */
export function isMine(entrants: readonly EntrantView[], match: MatchView): boolean {
  return entrants.some((e) => e.mine && (e.entrant === match.a || e.entrant === match.b))
}

/** The current round's matches. */
export function currentRound(tournament: TournamentView): MatchView[] {
  return tournament.rounds_played.find((r) => r.round === tournament.round)?.matches ?? []
}

/** Whether every match of the current round has a result, so the organiser can go on. */
export function roundDecided(tournament: TournamentView): boolean {
  return currentRound(tournament).every((m) => m.b === null || m.outcome !== null)
}

/** The link that signs a player up, on this page's origin. */
export function inviteLink(origin: string, invite: string): string {
  return `${origin}/tournaments/join/${encodeURIComponent(invite)}`
}

/** Entrants of a team format with an open place, for a player who joins one. */
export function openTeams(entrants: readonly EntrantView[], format: TournamentFormat): EntrantView[] {
  const size = format === 'commander_2v2' ? 2 : 1
  return entrants.filter((e) => e.players.length < size)
}

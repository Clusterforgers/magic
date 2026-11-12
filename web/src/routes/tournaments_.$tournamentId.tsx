import { createFileRoute } from '@tanstack/react-router'
import { TournamentRoute } from '@/features/tournaments/TournamentRoutes'

// The trailing underscore keeps this route out of `tournaments.tsx`, which is a page of its own and has no outlet.
export const Route = createFileRoute('/tournaments_/$tournamentId')({ component: () => <TournamentRoute /> })

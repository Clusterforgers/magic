import { createFileRoute } from '@tanstack/react-router'
import { JoinTournamentRoute } from '@/features/tournaments/TournamentRoutes'

export const Route = createFileRoute('/tournaments_/join/$code')({ component: () => <JoinTournamentRoute /> })

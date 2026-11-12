import { createFileRoute } from '@tanstack/react-router'
import { TournamentsPage } from '@/features/tournaments/TournamentsPage'

export const Route = createFileRoute('/tournaments')({ component: () => <TournamentsPage /> })

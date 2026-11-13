import { createFileRoute } from '@tanstack/react-router'
import { GamesPage } from '@/features/admin/GamesPage'

export const Route = createFileRoute('/admin/games')({ component: () => <GamesPage /> })

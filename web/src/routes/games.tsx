import { createFileRoute } from '@tanstack/react-router'
import { MyGamesPage } from '@/features/games/MyGamesPage'

export const Route = createFileRoute('/games')({ component: () => <MyGamesPage /> })

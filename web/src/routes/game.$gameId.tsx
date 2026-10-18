import { createFileRoute } from '@tanstack/react-router'
import { GameRoute } from '@/features/game/GameRoute'

export const Route = createFileRoute('/game/$gameId')({ component: GameRoute })

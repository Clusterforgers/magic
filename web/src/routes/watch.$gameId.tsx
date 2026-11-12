import { createFileRoute } from '@tanstack/react-router'
import { WatchRoute } from '@/features/game/WatchRoute'

export const Route = createFileRoute('/watch/$gameId')({ component: WatchRoute })

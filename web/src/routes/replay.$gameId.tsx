import { createFileRoute } from '@tanstack/react-router'
import { ReplayPage } from '@/features/replay/ReplayPage'

export const Route = createFileRoute('/replay/$gameId')({
  component: function ReplayRoute() {
    const { gameId } = Route.useParams()
    return <ReplayPage game={gameId} />
  },
})

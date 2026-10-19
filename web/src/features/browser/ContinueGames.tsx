import { Link } from '@tanstack/react-router'
import { PlayIcon } from 'lucide-react'
import { useEffect } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { buttonVariants } from '@/components/ui/button'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { othersText } from '@/features/games/format'

/**
 * "Continue": a link into each running game of the signed-in user, from the My games listing (one request when the
 * page opens). A guest, a user with no running game and a listing that fails show nothing; My games has the errors.
 */
export function ContinueGames() {
  const store = useAuthStore()
  const status = useAuth((state) => state.status)
  const running = useAuth(
    useShallow((state) => state.games.games.filter((game) => game.status === 'running')),
  )
  useEffect(() => {
    if (status === 'signed-in') void store.getState().loadGames()
  }, [status, store])
  if (running.length === 0) return null
  return (
    <section aria-label="Your running games" className="flex flex-wrap gap-2">
      {running.map((game) => {
        const others = othersText(game)
        return (
          <Link
            key={game.game}
            to="/game/$gameId"
            params={{ gameId: game.game }}
            className={buttonVariants({ variant: 'outline' })}
          >
            <PlayIcon aria-hidden="true" />
            {others === '' ? 'Continue your game' : `Continue your game with ${others}`}
          </Link>
        )
      })}
    </section>
  )
}

import { Link } from '@tanstack/react-router'
import { DoorOpenIcon, PlayIcon } from 'lucide-react'
import { useEffect } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { buttonVariants } from '@/components/ui/button'
import { useAuth, useAuthStore } from '@/features/auth/context'
import { continueText } from '@/features/games/format'

/**
 * "Continue": a link into each running game and each lobby the signed-in user holds a seat of, from the My games
 * listing (one request when the page opens), so a lobby left before everybody was ready is found again however full it
 * is. A guest, a user with no such game and a listing that fails show nothing; My games has the errors.
 */
export function ContinueGames() {
  const store = useAuthStore()
  const status = useAuth((state) => state.status)
  // Only a listing that has just answered counts: one still loading or failed may name a lobby that has since
  // closed (a Delves run or a tournament that ended), and a link back into it would open nothing.
  const open = useAuth(
    useShallow((state) =>
      state.games.status === 'ready'
        ? state.games.games.filter((game) => game.status === 'running' || game.status === 'lobby')
        : [],
    ),
  )
  useEffect(() => {
    if (status === 'signed-in') void store.getState().loadGames()
  }, [status, store])
  if (open.length === 0) return null
  return (
    <section aria-label="Your games" className="flex flex-wrap gap-2">
      {open.map((game) => {
        const Icon = game.status === 'lobby' ? DoorOpenIcon : PlayIcon
        return (
          <Link
            key={game.game}
            to="/game/$gameId"
            params={{ gameId: game.game }}
            data-continue={game.status}
            className={buttonVariants({ variant: 'outline' })}
          >
            <Icon aria-hidden="true" />
            {continueText(game)}
          </Link>
        )
      })}
    </section>
  )
}

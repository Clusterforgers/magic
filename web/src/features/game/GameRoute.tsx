import { getRouteApi, useLocation } from '@tanstack/react-router'
import { useShallow } from 'zustand/react/shallow'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'
import { fragmentOf, parseSeatFragment } from '@/lib/seatLink'
import { GamePage } from './GamePage'

const route = getRouteApi('/game/$gameId')

/** What the page shows, without a seat link, until the server has said who is signed in. */
function CheckingSession() {
  usePageTitle('Checking your sign-in')
  return (
    <PageShell>
      <Loading label="Checking your sign-in…">
        <Skeleton className="h-8 w-64" />
      </Loading>
    </PageShell>
  )
}

/**
 * The component of `/game/$gameId`: reads the game id from the path and the seat link from the fragment. With a seat
 * link the page joins by its token, as a guest does. Without one, a signed-in user joins through the session, which
 * the server maps to the seat saved to their account; anybody else is told what to do.
 */
export function GameRoute() {
  const { gameId } = route.useParams()
  // The router decodes `location.hash`; the raw address keeps the fragment exactly as it was written.
  const fragment = useLocation({ select: (location) => fragmentOf(location.href) })
  const link = parseSeatFragment(fragment)
  const { status, user } = useAuth(useShallow((state) => ({ status: state.status, user: state.user })))

  // No link yet: wait for the server to say who is signed in rather than show the wrong page for a moment.
  if (link === null && status === 'loading') return <CheckingSession />
  return <GamePage gameId={gameId} link={link} account={user?.username ?? null} />
}

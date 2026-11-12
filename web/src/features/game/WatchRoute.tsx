import { getRouteApi, Link } from '@tanstack/react-router'
import { useShallow } from 'zustand/react/shallow'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAuth } from '@/features/auth/context'
import type { AuthStatus } from '@/features/auth/store'
import { usePageTitle } from '@/hooks/usePageTitle'
import { GamePage } from './GamePage'
import type { GamePageProps } from './GamePage'

const route = getRouteApi('/watch/$gameId')

/** Why a visitor who is not signed in cannot watch, with the way to sign in and come back. */
function SignInToWatch({ gameId, accounts }: { gameId: string; accounts: boolean }) {
  usePageTitle('Sign in to watch')
  return (
    <PageShell>
      <h1 className="text-2xl font-semibold tracking-tight">Sign in to watch this game</h1>
      <p className="mt-2 max-w-prose text-sm text-muted-foreground">
        {accounts
          ? 'Anyone signed in may watch a running game. Spectators see what every player sees, never a hand.'
          : 'This server runs without accounts, so nobody can watch its games.'}
      </p>
      {accounts ? (
        <Link
          to="/login"
          search={{ next: `/watch/${gameId}` }}
          className={buttonVariants({ size: 'lg', className: 'mt-4' })}
        >
          Sign in
        </Link>
      ) : null}
    </PageShell>
  )
}

/** The props of `WatchPage`: the game, the sign-in state, and the page's dependencies for tests. */
export interface WatchPageProps extends Omit<GamePageProps, 'link' | 'watch' | 'account'> {
  status: AuthStatus
}

/** The spectator's page of a game: the table once the visitor is signed in, else the way to sign in. */
export function WatchPage({ status, gameId, ...rest }: WatchPageProps) {
  if (status === 'loading') {
    return (
      <PageShell>
        <Loading label="Checking your sign-in…">
          <Skeleton className="h-8 w-64" />
        </Loading>
      </PageShell>
    )
  }
  if (status !== 'signed-in') return <SignInToWatch gameId={gameId} accounts={status === 'signed-out'} />
  return <GamePage gameId={gameId} link={null} watch {...rest} />
}

/**
 * The component of `/watch/$gameId`: any signed-in visitor watches a running game as a spectator, with no seat and
 * no link; the session cookie of the socket says who it is.
 */
export function WatchRoute() {
  const { gameId } = route.useParams()
  const status = useAuth(useShallow((state) => state.status))
  return <WatchPage gameId={gameId} status={status} />
}

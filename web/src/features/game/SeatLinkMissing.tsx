import { Link } from '@tanstack/react-router'
import { PageShell } from '@/components/PageShell'
import { buttonVariants } from '@/components/ui/button'
import { useAuth } from '@/features/auth/context'
import { usePageTitle } from '@/hooks/usePageTitle'

/**
 * Shown when the address has no readable seat and token and nobody is signed in: nothing is connected until the
 * player opens their link, or signs in to rejoin a seat that is saved to their account.
 */
export function SeatLinkMissing({ gameId }: { gameId?: string }) {
  // Only a server with accounts can offer the second way in; a signed-in visitor never gets here.
  const canSignIn = useAuth((state) => state.status === 'signed-out')
  const title = canSignIn ? 'Open your seat link or sign in' : 'Open your seat link'
  usePageTitle(title)
  return (
    <PageShell className="grid gap-4">
      <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
      <p className="text-muted-foreground">
        This page needs the link made for your seat. It ends in something like{' '}
        <code className="rounded-sm bg-muted px-1 py-1 text-sm">#seat=1&amp;token=…</code>, and the host gets
        it when the game is created.
      </p>
      <p className="text-muted-foreground">
        Open that link, or ask the host to send it again. If you did open a seat link, it may have been cut
        short when it was copied.
      </p>
      {canSignIn ? (
        <p className="text-muted-foreground">
          If you played this game with an account, sign in instead: your seat is saved to the account and
          needs no link.
        </p>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {canSignIn ? (
          <Link
            to="/login"
            search={gameId === undefined ? {} : { next: `/game/${encodeURIComponent(gameId)}` }}
            className={buttonVariants()}
          >
            Sign in
          </Link>
        ) : null}
        <Link to="/new" className={buttonVariants({ variant: canSignIn ? 'outline' : 'default' })}>
          Create a new game
        </Link>
      </div>
    </PageShell>
  )
}

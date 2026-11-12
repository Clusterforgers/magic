import { getRouteApi, useLocation } from '@tanstack/react-router'
import { PageShell } from '@/components/PageShell'
import { usePageTitle } from '@/hooks/usePageTitle'
import { fragmentOf, parseWatchFragment } from '@/lib/seatLink'
import { GamePage } from './GamePage'

const route = getRouteApi('/watch/$gameId')

/** A watch link without a readable token: nothing to open. */
function WatchLinkMissing() {
  usePageTitle('Watch link incomplete')
  return (
    <PageShell>
      <h1 className="text-2xl font-semibold tracking-tight">This watch link is incomplete</h1>
      <p className="mt-2 max-w-prose text-sm text-muted-foreground">
        Ask a player of the game for a new watch link, and open it whole.
      </p>
    </PageShell>
  )
}

/**
 * The component of `/watch/$gameId`: a spectator's page. The token of the watch link is in the fragment, which a
 * browser never sends; the page watches the game through it and holds no seat.
 */
export function WatchRoute() {
  const { gameId } = route.useParams()
  // The router decodes `location.hash`; the raw address keeps the fragment exactly as it was written.
  const fragment = useLocation({ select: (location) => fragmentOf(location.href) })
  return <WatchPage gameId={gameId} fragment={fragment} />
}

/** The spectator's page of `gameId` for the fragment of its address: the game, or why the link opens nothing. */
export function WatchPage({ gameId, fragment }: { gameId: string; fragment: string }) {
  const token = parseWatchFragment(fragment)
  if (token === null) return <WatchLinkMissing />
  return <GamePage gameId={gameId} link={null} watch={token} />
}

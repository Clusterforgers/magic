import { getRouteApi, useLocation } from '@tanstack/react-router'
import { fragmentOf, parseSeatFragment } from '@/lib/seatLink'
import { GamePage } from './GamePage'

const route = getRouteApi('/game/$gameId')

/** The component of `/game/$gameId`: reads the game id from the path and the seat link from the fragment. */
export function GameRoute() {
  const { gameId } = route.useParams()
  // The router decodes `location.hash`; the raw address keeps the fragment exactly as it was written.
  const fragment = useLocation({ select: (location) => fragmentOf(location.href) })
  return <GamePage gameId={gameId} link={parseSeatFragment(fragment)} />
}

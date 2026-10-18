import { useLocation, useParams } from '@tanstack/react-router'

/** Stands in for the game page in a test router: shows the id and the raw address so a test can see where a click led. */
export function GameStub() {
  const { gameId } = useParams({ strict: false })
  const href = useLocation({ select: (location) => location.href })
  return <p data-testid="game-stub">{`${gameId ?? '?'} ${href}`}</p>
}

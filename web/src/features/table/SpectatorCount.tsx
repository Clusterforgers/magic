import { EyeIcon } from 'lucide-react'
import { usePlay } from '@/features/game/gameContext'

/** "1 person is watching" or "3 people are watching". */
export function watchingText(count: number): string {
  return count === 1 ? '1 person is watching' : `${count} people are watching`
}

/**
 * For a player, how many spectators watch the game: an eye and the count in the top bar, named in full for a screen
 * reader and on hover. Nothing while nobody watches, and nothing for a spectator.
 */
export function SpectatorCount() {
  const count = usePlay((state) => (state.view?.spectator === true ? 0 : state.spectators))
  if (count === 0) return null
  const words = watchingText(count)
  return (
    <span
      data-slot="spectator-count"
      role="status"
      aria-label={words}
      title={words}
      className="inline-flex shrink-0 items-center gap-1 rounded-md px-1.5 text-xs text-muted-foreground tabular-nums"
    >
      <EyeIcon aria-hidden="true" className="size-4" />
      {count}
    </span>
  )
}

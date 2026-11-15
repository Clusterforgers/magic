import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { MulliganStatus } from '@/protocol/generated'
import { useSeatNamer } from './names'
import { useOpeningInOverlay } from './prompt/choiceOverlay'
import { describeOpening } from './turnText'

/** How a status groups for the eye: still choosing, done, or going again. */
function kindOf(status: MulliganStatus): 'choosing' | 'done' | 'again' {
  if (status === 'Deciding' || status === 'Opening') return 'choosing'
  if (status === 'Kept' || typeof status !== 'string') return 'done'
  return 'again'
}

/**
 * During the opening hands, one line that lists every seat with what it is doing: deciding, kept (and how many
 * cards it still bottoms), taking a mulligan, and how many mulligans it has taken. It is the public part of what the
 * seat panels say, in one place, so a table of any size shows it at a glance. Nothing before or after that stage.
 */
export function OpeningStatus({ floating = false }: { floating?: boolean } = {}) {
  const name = useSeatNamer()
  const stage = usePlay((state) => state.view?.stage)
  const players = usePlay((state) => state.view?.players)
  const me = usePlay((state) => state.seat)
  // The floating decision carries the list itself; the strip leaves it out while that is open.
  const carried = useOpeningInOverlay()
  if (stage !== 'Mulligan' || players === undefined || (carried && !floating)) return null
  return (
    <section
      aria-label="Opening hand decisions"
      data-slot="opening-status"
      className={floating ? 'text-center' : 'basis-full'}
    >
      <ul className={cn('flex flex-wrap gap-x-5 gap-y-1 text-sm', floating && 'justify-center')}>
        {players.map((player) => {
          const kind = kindOf(player.mulligan)
          return (
            <li
              key={player.seat}
              data-seat-status={player.seat}
              data-status={kind}
              className="min-w-0 [overflow-wrap:anywhere]"
            >
              <span className="font-medium">{player.seat === me ? 'You' : name(player.seat)}:</span>{' '}
              <span className={cn(kind === 'choosing' ? 'text-foreground' : 'text-muted-foreground')}>
                {describeOpening(player.mulligan, player.mulligans)}
              </span>
            </li>
          )
        })}
      </ul>
    </section>
  )
}

/**
 * A reworked boss's phase pill in the top bar, beside the round it counts: "Phase 1 in 3 rounds" until the phase
 * begins, then the phase's name with the countdown to the next; a narrow window says it short ("P1 in 3"). Hover or
 * focus shows the phase it names in the card preview and a press pins it there, where the boss's phases are steps to
 * move between. The pill keeps one width whatever it says, so nothing after it moves.
 */
import { FlameIcon, HourglassIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { BossView, PlayerView } from '@/protocol/generated'
import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '../names'
import { bossAbilities } from './bossAbilities'
import { useCustomPreview } from './useCustomPreview'
import { phaseCountdown } from './reworkedBoss'

/** The first boss seat with phases by round, whose pill the top bar shows; none at any other table. */
function phasedBoss(players: readonly PlayerView[] | undefined): PlayerView | undefined {
  return players?.find((player) => (player.boss?.round_phases?.length ?? 0) > 0)
}

/** The top bar's phase pill of the boss with phases by round; nothing at a table without one. */
export function BossPhasePill() {
  const player = usePlay((state) => phasedBoss(state.view?.players))
  if (player?.boss == null) return null
  return <PhasePill player={player} boss={player.boss} />
}

function PhasePill({ player, boss }: { player: PlayerView; boss: BossView }) {
  const countdown = phaseCountdown(boss)
  const owner = useSeatNamer()(player.seat)
  const all = useMemo(() => bossAbilities(player), [player])
  const phases = all.filter((ability) => ability.kind === 'phase' || ability.kind === 'next-phase')
  const named = phases[countdown?.named ?? 0]
  const handlers = useCustomPreview(named, all, owner)
  if (countdown === null || handlers === null) return null
  const Icon = countdown.begun ? FlameIcon : HourglassIcon
  return (
    <button
      type="button"
      {...handlers}
      data-slot="phase-countdown"
      data-begun={countdown.begun || undefined}
      aria-label={countdown.title}
      className={cn(
        'flex h-7 w-20 shrink-0 items-center gap-1 rounded-full border px-1.5 text-left text-xs font-semibold outline-none hover:brightness-110 focus-visible:ring-2 focus-visible:ring-ring xl:w-40 xl:px-2 2xl:w-72',
        countdown.begun ? 'border-boss bg-boss text-on-boss' : 'border-boss/60 bg-boss/10 text-foreground',
      )}
    >
      <Icon
        aria-hidden="true"
        className={cn('size-3.5 shrink-0', countdown.begun ? 'text-on-boss' : 'text-boss')}
      />
      <span aria-hidden="true" data-slot="phase-countdown-short" className="min-w-0 truncate xl:hidden">
        {countdown.short}
      </span>
      <span aria-hidden="true" className="hidden min-w-0 truncate xl:inline">
        {countdown.label}
      </span>
      {countdown.next === undefined ? null : (
        <span aria-hidden="true" className="hidden min-w-0 truncate font-medium opacity-80 2xl:inline">
          · {countdown.next}
        </span>
      )}
    </button>
  )
}

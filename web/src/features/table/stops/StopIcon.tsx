import {
  Crosshair,
  Flag,
  Flame,
  Hourglass,
  Moon,
  PlayingCard,
  Shield,
  Sparkles,
  Swords,
  Zap,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { StopStep } from '@/play/stops'

/** The picture of each step; the two main phases share one and tell apart by a numeral. */
const ICONS: Record<StopStep, { icon: LucideIcon; mark?: string }> = {
  Upkeep: { icon: Hourglass },
  Draw: { icon: PlayingCard },
  Main1: { icon: Sparkles, mark: '1' },
  BeginCombat: { icon: Crosshair },
  DeclareAttackers: { icon: Swords },
  DeclareBlockers: { icon: Shield },
  FirstStrikeDamage: { icon: Zap },
  CombatDamage: { icon: Flame },
  EndCombat: { icon: Flag },
  Main2: { icon: Sparkles, mark: '2' },
  End: { icon: Moon },
}

/** The picture of a step for its button; decoration, the button carries the words. */
export function StopIcon({ step }: { step: StopStep }) {
  const { icon: Icon, mark } = ICONS[step]
  return (
    <span aria-hidden="true" className="relative inline-flex items-center justify-center">
      <Icon className="size-4" />
      {mark === undefined ? null : (
        <span className="absolute -right-1.5 -bottom-1 text-[0.6875rem] leading-none font-bold">{mark}</span>
      )}
    </span>
  )
}

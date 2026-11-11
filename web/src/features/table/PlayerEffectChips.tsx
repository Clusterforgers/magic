/**
 * The effects on a seat on its pod: one chip per emblem and per rule in force on the player (can't lose, hexproof,
 * no maximum hand size), each an icon and a few words, with what it does on hover or focus. A tile, which has no room
 * for words, shows one figure with the count and every effect in its detail.
 */
import { Award, Ban, Eye, Hand, Heart, Mountain, Shield, Sparkles } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerView } from '@/protocol/generated'
import { DENSITY } from './PodParts'
import type { ChipDensity } from './PodParts'
import { effectsName, seatEffects } from './playerEffects'
import type { EffectFacts, EffectIcon, EffectTone } from './playerEffects'
import { useStable } from './useStable'

const ICON: Record<EffectIcon, LucideIcon> = {
  shield: Shield,
  ban: Ban,
  heart: Heart,
  eye: Eye,
  hand: Hand,
  land: Mountain,
  emblem: Award,
  keyword: Sparkles,
}

// The tone tints the icon only; the words on the chip say the effect, so the colour is never the only cue.
const TONE: Record<EffectTone, string> = {
  help: 'text-success',
  hinder: 'text-destructive',
  neutral: 'text-muted-foreground',
}

function EffectChip({ effect, density }: { effect: EffectFacts; density: Exclude<ChipDensity, 'tile'> }) {
  const Icon = ICON[effect.icon]
  const chip = (
    <span
      role="img"
      aria-label={effect.detail}
      tabIndex={0}
      data-slot="player-effect"
      data-effect={effect.key}
      data-tone={effect.tone}
      className={cn(DENSITY[density].chip, 'max-w-36 text-foreground')}
    >
      <Icon aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon, TONE[effect.tone])} />
      <span aria-hidden="true" className="truncate">
        {effect.label}
      </span>
    </span>
  )
  return (
    <Tooltip>
      <TooltipTrigger render={chip} />
      <TooltipContent side="bottom" className="max-w-64">
        {effect.detail}
      </TooltipContent>
    </Tooltip>
  )
}

/** Every effect on `player`; nothing while there is none. */
export function PlayerEffectChips({
  owner,
  player,
  density = 'line',
}: {
  owner: string
  player: PlayerView
  density?: ChipDensity
}) {
  const effects = usePlay(useStable((state) => seatEffects(player, state.objectsById, owner)))
  if (effects.length === 0) return null
  if (density !== 'tile') {
    return effects.map((effect) => <EffectChip key={effect.key} effect={effect} density={density} />)
  }
  // A tile is one button: the figure is no control of its own, and the tile's name says the effects.
  return (
    <span
      role="img"
      aria-label={effectsName(owner, effects)}
      title={effects.map((effect) => effect.detail).join('\n')}
      data-slot="player-effects"
      className={cn(DENSITY.tile.chip, 'text-muted-foreground')}
    >
      <Sparkles aria-hidden="true" className={cn('shrink-0', DENSITY.tile.icon)} />
      <span aria-hidden="true" className="tabular-nums">
        {effects.length}
      </span>
    </span>
  )
}

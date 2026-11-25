/**
 * The effects on a seat on its pod: one chip per emblem and per rule in force on the player (can't lose, hexproof,
 * no maximum hand size), each an icon and a few words, with what it does on hover or focus. A tile, which has no room
 * for words, shows one figure with the count and every effect in its detail.
 */
import { Award, Ban, Crown, Eye, Ghost, Hand, Heart, Mountain, Shield, Sparkles } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { Color, ObjectId, PlayerView } from '@/protocol/generated'
import { costPips } from './cardModel'
import { Pip } from './ManaPips'
import { DENSITY } from './PodParts'
import type { ChipDensity } from './PodParts'
import { useContext } from 'react'
import { BossBarContext } from './campaign/useBossAbilities'
import { effectsName, seatEffects, withoutBossBar } from './playerEffects'
import type { EffectFacts, EffectIcon, EffectTone } from './playerEffects'
import { useStable } from './useStable'
import { usePreviewHandlers } from './usePreviewHandlers'
import { useSeatNamer } from './names'

const ICON: Record<EffectIcon, LucideIcon> = {
  shield: Shield,
  ban: Ban,
  heart: Heart,
  eye: Eye,
  hand: Hand,
  land: Mountain,
  emblem: Award,
  keyword: Sparkles,
  spirit: Ghost,
  boss: Crown,
}

// The tone tints the icon only; the words on the chip say the effect, so the colour is never the only cue.
const TONE: Record<EffectTone, string> = {
  help: 'text-success',
  hinder: 'text-destructive',
  neutral: 'text-muted-foreground',
}

/** Each colour of a seat's costs with the one it stands for this round, as pips: "(R)→(G)". */
function DrawnColors({ colors }: { colors: readonly (readonly [Color, Color])[] }) {
  const pip = (color: Color) => costPips([{ Colored: color }])[0]
  return (
    <span aria-hidden="true" data-slot="drawn-colors" className="flex items-center gap-1.5">
      {colors.map(([from, to]) => {
        const [a, b] = [pip(from), pip(to)]
        return a === undefined || b === undefined ? null : (
          <span key={from} className="flex items-center gap-px">
            <Pip pip={a} sizeClass="size-4 text-xs" />
            <span className="text-xs text-muted-foreground">→</span>
            <Pip pip={b} sizeClass="size-4 text-xs" />
          </span>
        )
      })}
    </span>
  )
}

/**
 * An emblem's chip: a button whose hover or focus shows the emblem in the card preview, a relic of a Delves fight in
 * its frame, and whose press pins it there.
 */
function EmblemChip({
  effect,
  object,
  density,
}: {
  effect: EffectFacts
  object: ObjectId
  density: Exclude<ChipDensity, 'tile'>
}) {
  const Icon = ICON[effect.icon]
  const handlers = usePreviewHandlers(object)
  return (
    <button
      type="button"
      {...handlers}
      aria-label={effect.detail}
      data-object-id={object}
      data-slot="player-effect"
      data-effect={effect.key}
      data-tone={effect.tone}
      className={cn(DENSITY[density].chip, 'max-w-36 text-foreground hover:brightness-110')}
    >
      <Icon aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon, TONE[effect.tone])} />
      <span aria-hidden="true" className="truncate">
        {effect.label}
      </span>
    </button>
  )
}

function EffectChip({ effect, density }: { effect: EffectFacts; density: Exclude<ChipDensity, 'tile'> }) {
  if (effect.object !== undefined)
    return <EmblemChip effect={effect} object={effect.object} density={density} />
  const Icon = ICON[effect.icon]
  const chip = (
    <span
      role="img"
      aria-label={effect.detail}
      tabIndex={0}
      data-slot="player-effect"
      data-effect={effect.key}
      data-tone={effect.tone}
      className={cn(DENSITY[density].chip, effect.colors === undefined && 'max-w-36', 'text-foreground')}
    >
      <Icon aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon, TONE[effect.tone])} />
      {effect.colors === undefined ? (
        <span aria-hidden="true" className="truncate">
          {effect.label}
        </span>
      ) : (
        <DrawnColors colors={effect.colors} />
      )}
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
  const namer = useSeatNamer()
  const barred = useContext(BossBarContext) && player.boss != null
  const effects = usePlay(
    useStable((state) => {
      const all = seatEffects(player, state.objectsById, owner, namer)
      return barred ? withoutBossBar(all) : all
    }),
  )
  if (effects.length === 0) return null
  if (density !== 'tile') {
    return effects.map((effect) => <EffectChip key={effect.key} effect={effect} density={density} />)
  }
  // A tile is one button: the figure is no control of its own, and the tile's name says the effects. The colours a
  // hero's costs stand for this round stay in sight as pips, since every cast hangs on them.
  const drawn = effects.find((effect) => effect.colors !== undefined)
  const others = effects.filter((effect) => effect !== drawn)
  return (
    <>
      {drawn?.colors === undefined ? null : (
        <span title={drawn.detail} className={DENSITY.tile.chip}>
          <DrawnColors colors={drawn.colors} />
        </span>
      )}
      {others.length === 0 ? null : <EffectCount owner={owner} effects={effects} count={others.length} />}
    </>
  )
}

/** A tile's figure for its effects: the count, with every effect named in the tile. */
function EffectCount({
  owner,
  effects,
  count,
}: {
  owner: string
  effects: readonly EffectFacts[]
  count: number
}) {
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
        {count}
      </span>
    </span>
  )
}

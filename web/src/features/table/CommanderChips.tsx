import { Crown, Swords } from 'lucide-react'
import { useMemo, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId, ObjectView, PlayerId, PlayerView } from '@/protocol/generated'
import { damageFraction, damageLevel, damageWords, shortName, taxChip, taxWords } from './commander'
import type { DamageLevel } from './commander'
import { CHIP } from './PodParts'
import type { ChipDensity } from './PodParts'
import { playerOf } from './selectors'

type Objects = Readonly<Record<ObjectId, ObjectView>>

/** One chip of a commander: its text, its spoken words and, for damage, how close it is to lethal. */
export interface CommanderChip {
  key: number
  text: string
  /** The figure alone, for a corner with no room for a name: "+4" for tax, "14/21" for damage. */
  figure: string
  words: string
  level?: DamageLevel
}

/** The look of a damage chip by level: muted, the warning tone from 15 of 21, red from 18 and filled red at lethal. */
export const LEVEL_TONE: Record<DamageLevel, string> = {
  plain: 'bg-muted text-muted-foreground',
  warn: 'bg-warning/15 text-warning ring-1 ring-warning/40',
  danger: 'bg-destructive/15 text-destructive ring-1 ring-destructive/50',
  lethal: 'bg-destructive text-white',
}

const SHAPE: Record<ChipDensity, string> = {
  panel: cn(CHIP, 'justify-start px-1.5'),
  line: cn(CHIP, 'gap-0.5 px-1'),
  tile: 'inline-flex shrink-0 items-center gap-0.5 rounded px-0.5 text-xs font-medium',
}

/** The tax chips of a seat: one per commander that was cast or carries tax, named after it when the seat has two. */
export function taxChips(player: PlayerView | undefined, objects: Objects): CommanderChip[] {
  const commanders = player?.commanders ?? []
  return commanders.flatMap((commander): CommanderChip[] => {
    const text = taxChip(commander)
    if (text === null) return []
    const name = objects[commander.object]?.name ?? 'Commander'
    return [
      {
        key: commander.object,
        text: commanders.length > 1 ? `${shortName(name)} · ${text}` : text,
        figure: commander.tax > 0 ? `+${commander.tax}` : `${commander.casts}×`,
        words: taxWords(name, commander),
      },
    ]
  })
}

/**
 * The commander damage chips of the seat that took it: "Atraxa 14/21", one per commander, highest first. A commander
 * the viewer cannot see now (in a hand or a library) keeps the name `seen` holds for it.
 */
export function damageChips(
  player: PlayerView | undefined,
  objects: Objects,
  seen: ReadonlyMap<ObjectId, string> = new Map(),
): CommanderChip[] {
  const lethal = player?.commander_lethal
  return [...(player?.commander_damage ?? [])]
    .sort((a, b) => b.damage - a.damage || a.commander - b.commander)
    .map((entry): CommanderChip => {
      const name = objects[entry.commander]?.name ?? seen.get(entry.commander) ?? 'A commander'
      return {
        key: entry.commander,
        text: `${shortName(name)} ${damageFraction(entry.damage, lethal)}`,
        figure: damageFraction(entry.damage, lethal),
        words: damageWords(name, entry.damage, lethal),
        level: damageLevel(entry.damage, lethal),
      }
    })
}

/** The tax and damage chips of a seat, worked out again only when its view or the objects change. */
export function useCommanderChips(seat: PlayerId): { tax: CommanderChip[]; damage: CommanderChip[] } {
  const player = usePlay((state) => playerOf(state, seat))
  const objects = usePlay((state) => state.objectsById)
  const seen = useSeenNames(player, objects)
  return useMemo(
    () => ({ tax: taxChips(player, objects), damage: damageChips(player, objects, seen) }),
    [player, objects, seen],
  )
}

/**
 * The names of the commanders that dealt this seat damage, as the page last saw them: a commander that goes to a hand
 * or a library leaves the viewer's objects, but its name is public.
 */
function useSeenNames(player: PlayerView | undefined, objects: Objects): ReadonlyMap<ObjectId, string> {
  const [seen, setSeen] = useState<ReadonlyMap<ObjectId, string>>(() => new Map())
  const fresh = (player?.commander_damage ?? []).flatMap(({ commander }) => {
    const name = objects[commander]?.name
    return name === undefined || seen.get(commander) === name ? [] : [[commander, name] as const]
  })
  // Kept while rendering, as React allows for state that follows props: the next render has the name at once.
  if (fresh.length > 0) setSeen(new Map([...seen, ...fresh]))
  return seen
}

interface ChipsProps {
  chips: readonly CommanderChip[]
  density: ChipDensity
  className?: string
  /** Write the figure alone ("14/21"); the name stays in the chip's spoken name and tooltip. */
  compact?: boolean
}

/** Tax chips, each an image named in words ("Atraxa: cast 2 times from the command zone, tax 4 mana"). */
export function TaxChips({ chips, density, className, compact = false }: ChipsProps) {
  return chips.map((chip) => (
    <span
      key={chip.key}
      role="img"
      aria-label={chip.words}
      title={chip.words}
      data-slot="commander-tax"
      className={cn(
        SHAPE[density],
        density !== 'tile' && 'bg-muted',
        'min-w-0 text-muted-foreground',
        className,
      )}
    >
      <Crown aria-hidden="true" className="size-3.5 shrink-0" />
      <span aria-hidden="true" className="truncate tabular-nums">
        {compact ? chip.figure : chip.text}
      </span>
    </span>
  ))
}

/** Commander damage chips, toned by how close each is to lethal and named in words. */
export function DamageChips({ chips, density, className, compact = false }: ChipsProps) {
  return chips.map((chip) => {
    const level = chip.level ?? 'plain'
    return (
      <span
        key={chip.key}
        role="img"
        aria-label={chip.words}
        title={chip.words}
        data-slot="commander-damage"
        data-level={level}
        className={cn(
          SHAPE[density],
          'min-w-0',
          LEVEL_TONE[level],
          level !== 'plain' && 'font-semibold',
          className,
        )}
      >
        <Swords aria-hidden="true" className="size-3.5 shrink-0" />
        <span aria-hidden="true" className="truncate tabular-nums">
          {compact ? chip.figure : chip.text}
        </span>
      </span>
    )
  })
}

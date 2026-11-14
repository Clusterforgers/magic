/**
 * The boss's abilities across the top of a fight against a boss: one chip each with its name and a one-line summary,
 * always visible, and the whole rules text on hover, focus or press. The intro screen of a Delves fight lists the same
 * abilities in full (`BossAbilityList`).
 */
import { CrownIcon, EyeIcon, FlameIcon, HourglassIcon, ScrollTextIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { useSeatNamer } from '../names'
import type { BossAbility, BossAbilityKind } from './bossAbilities'
import { KIND_WORDS, useBossAbilities } from './useBossAbilities'

const ICONS: Record<BossAbilityKind, LucideIcon> = {
  passive: CrownIcon,
  phase: FlameIcon,
  'next-phase': HourglassIcon,
  scheme: ScrollTextIcon,
  omen: EyeIcon,
}

/** One ability as a chip: its kind's icon, name and summary; the whole text on hover, focus or press. */
function AbilityChip({ ability }: { ability: BossAbility }) {
  const Icon = ICONS[ability.kind]
  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        delay={200}
        data-slot="boss-ability"
        data-kind={ability.kind}
        className={cn(
          'flex h-8 max-w-72 min-w-0 shrink-0 items-center gap-1.5 rounded-full border px-2 text-left text-xs',
          ability.kind === 'omen' || ability.kind === 'next-phase'
            ? 'border-dashed border-boss/70 bg-card'
            : 'border-boss bg-card',
        )}
      >
        <Icon aria-hidden="true" className="size-3.5 shrink-0 text-boss" />
        <span className="shrink-0 font-semibold">{ability.name}</span>
        {ability.summary === '' ? null : (
          <span className="truncate text-muted-foreground">{ability.summary}</span>
        )}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 gap-1 p-3 text-sm">
        <p className="text-xs font-medium text-boss">
          {KIND_WORDS[ability.kind]}
          {ability.when === undefined ? '' : ` · ${ability.when}`}
        </p>
        <PopoverTitle>{ability.name}</PopoverTitle>
        <p data-slot="boss-ability-text" className="whitespace-pre-line">
          {ability.text === '' ? 'Its rules are being read…' : ability.text}
        </p>
      </PopoverContent>
    </Popover>
  )
}

/** The boss's abilities as one row of chips, scrolling sideways when they do not fit. */
export function BossAbilitiesBar({ seat }: { seat: PlayerId }) {
  const abilities = useBossAbilities(seat)
  const name = useSeatNamer()(seat)
  if (abilities.length === 0) return null
  return (
    <ul
      aria-label={`${name}'s abilities`}
      data-slot="boss-abilities"
      className="flex h-9 shrink-0 items-center gap-1.5 overflow-x-auto px-1 [scrollbar-width:thin]"
    >
      {abilities.map((ability) => (
        <li key={ability.key} className="flex min-w-0 shrink-0">
          <AbilityChip ability={ability} />
        </li>
      ))}
    </ul>
  )
}

/** The abilities in full, for the intro screen of a fight: each with its kind, when it comes and its whole text. */
export function BossAbilityList({ abilities }: { abilities: readonly BossAbility[] }) {
  return (
    <ul aria-label="Boss abilities" className="grid gap-2 sm:grid-cols-2">
      {abilities.map((ability) => {
        const Icon = ICONS[ability.kind]
        return (
          <li
            key={ability.key}
            data-slot="boss-ability"
            className="grid gap-1 rounded-lg border bg-surface p-3"
          >
            <p className="flex items-center gap-1.5 text-xs font-medium text-boss">
              <Icon aria-hidden="true" className="size-3.5" />
              {KIND_WORDS[ability.kind]}
              {ability.when === undefined ? '' : ` · ${ability.when}`}
            </p>
            <p className="text-sm font-semibold">{ability.name}</p>
            <p className="text-sm whitespace-pre-line text-muted-foreground">{ability.text}</p>
          </li>
        )
      })}
    </ul>
  )
}

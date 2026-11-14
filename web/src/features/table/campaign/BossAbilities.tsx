/**
 * The boss's abilities across the top of a fight against a boss: one row of small chips, each its kind's icon and its
 * name, the whole rules text on hover, focus or press. The row never wraps: chips that do not fit are left out of it and
 * counted on the "All" button at its end, which lists every ability in full. The intro screen of a Delves fight lists
 * the same abilities (`BossAbilityList`).
 */
import { CrownIcon, EyeIcon, FlameIcon, HourglassIcon, ListIcon, ScrollTextIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { usePlay } from '@/features/game/gameContext'
import { useSeatNamer } from '../names'
import { playerOf } from '../selectors'
import { barOrder } from './bossAbilities'
import { phaseSteps } from './bossPhase'
import { BossTrack } from './CampaignMarks'
import type { BossAbility, BossAbilityKind } from './bossAbilities'
import { KIND_WORDS, useBossAbilities } from './useBossAbilities'

const ICONS: Record<BossAbilityKind, LucideIcon> = {
  passive: CrownIcon,
  phase: FlameIcon,
  'next-phase': HourglassIcon,
  scheme: ScrollTextIcon,
  omen: EyeIcon,
}

/** Each kind's chip: the phase in force filled, the scheme tinted, what is always on plain, what is to come dashed. */
const CHIP: Record<BossAbilityKind, { chip: string; icon: string }> = {
  phase: { chip: 'border-boss bg-boss text-on-boss', icon: 'text-on-boss' },
  scheme: { chip: 'border-boss bg-boss/15', icon: 'text-boss' },
  passive: { chip: 'border-boss/45 bg-card', icon: 'text-boss' },
  'next-phase': { chip: 'border-dashed border-boss/60 bg-card text-muted-foreground', icon: 'text-boss' },
  omen: { chip: 'border-dashed border-boss/60 bg-card text-muted-foreground', icon: 'text-boss' },
}

/** One ability's kind, when it comes, name and whole text: the body of its popover and a row of the full list. */
function AbilityText({ ability, title }: { ability: BossAbility; title: boolean }) {
  const Icon = ICONS[ability.kind]
  return (
    <>
      <p className="flex items-center gap-1.5 text-xs font-medium text-boss">
        <Icon aria-hidden="true" className="size-3.5 shrink-0" />
        {KIND_WORDS[ability.kind]}
        {ability.when === undefined ? '' : ` · ${ability.when}`}
      </p>
      {title ? (
        <PopoverTitle>{ability.name}</PopoverTitle>
      ) : (
        <p className="text-sm font-semibold">{ability.name}</p>
      )}
      <p data-slot="boss-ability-text" className="text-sm whitespace-pre-line text-muted-foreground">
        {ability.text === '' ? 'Its rules are being read…' : ability.text}
      </p>
    </>
  )
}

/** One ability as a chip: its kind's icon and name, its summary for a screen reader; the whole text on hover, focus or press. */
function AbilityChip({ ability, hidden }: { ability: BossAbility; hidden: boolean }) {
  const Icon = ICONS[ability.kind]
  const tone = CHIP[ability.kind]
  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        delay={200}
        tabIndex={hidden ? -1 : undefined}
        data-slot="boss-ability"
        data-kind={ability.kind}
        className={cn(
          'flex h-7 max-w-48 min-w-0 items-center gap-1 rounded-full border px-2 text-left text-xs font-semibold transition-colors outline-none hover:brightness-110 focus-visible:ring-2 focus-visible:ring-ring',
          tone.chip,
        )}
      >
        <Icon aria-hidden="true" className={cn('size-3.5 shrink-0', tone.icon)} />
        <span className="truncate">{ability.name}</span>
        {ability.summary === '' ? null : <span className="sr-only"> {ability.summary}</span>}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 gap-1 p-3 text-sm">
        <AbilityText ability={ability} title />
      </PopoverContent>
    </Popover>
  )
}

/**
 * How many children of `ref` lie under its first line, the ones a list of one line clips; zero while it is not
 * measured (a test, a hidden box).
 */
function useWrapped(ref: RefObject<HTMLElement | null>, count: number): number {
  const [wrapped, setWrapped] = useState(0)
  useLayoutEffect(() => {
    const box = ref.current
    if (box === null) return
    const measure = (): void => {
      const items = Array.from(box.children) as HTMLElement[]
      const top = items[0]?.offsetTop ?? 0
      setWrapped(items.filter((item) => item.offsetTop > top).length)
    }
    measure()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(box)
    return () => observer.disconnect()
  }, [ref, count])
  return wrapped
}

/** The button at the end of the row: every ability in full, with the count of those the row has no room for. */
function AllAbilities({
  name,
  abilities,
  hidden,
}: {
  name: string
  abilities: BossAbility[]
  hidden: number
}) {
  return (
    <Popover>
      <PopoverTrigger
        data-slot="boss-abilities-all"
        aria-label={`All ${abilities.length} abilities of ${name}`}
        className="flex h-7 shrink-0 items-center gap-1 rounded-full border border-boss/45 bg-card px-2 text-xs font-semibold text-foreground outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
      >
        <ListIcon aria-hidden="true" className="size-3.5 text-boss" />
        <span aria-hidden="true">{hidden > 0 ? `+${hidden}` : 'All'}</span>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="max-h-[min(32rem,70dvh)] w-96 max-w-[calc(100vw-2rem)] gap-0 overflow-y-auto p-0 text-sm"
      >
        <PopoverTitle className="sticky top-0 border-b bg-popover px-3 py-2">{name}'s abilities</PopoverTitle>
        <ul className="grid divide-y">
          {abilities.map((ability) => (
            <li key={ability.key} data-slot="boss-ability-full" className="grid gap-0.5 px-3 py-2">
              <AbilityText ability={ability} title={false} />
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  )
}

/**
 * The boss's abilities as one row of chips, the phase in force and the scheme first; what does not fit is counted on
 * the button at the end, which lists them all.
 */
export function BossAbilitiesBar({ seat }: { seat: PlayerId }) {
  const abilities = barOrder(useBossAbilities(seat))
  const name = useSeatNamer()(seat)
  const boss = usePlay((state) => playerOf(state, seat)?.boss ?? null)
  const list = useRef<HTMLUListElement>(null)
  const hidden = useWrapped(list, abilities.length)
  if (abilities.length === 0) return null
  const shown = abilities.length - hidden
  const step = boss === null ? undefined : phaseSteps(boss).find((each) => each.state === 'current')
  return (
    <div data-slot="boss-abilities" className="flex h-8 min-w-0 shrink-0 items-center gap-1.5 px-1">
      {boss === null || boss.phases.length === 0 ? null : (
        <div
          data-slot="boss-bar-track"
          className="flex h-7 shrink-0 items-center gap-1.5 rounded-full bg-boss/10 px-2"
        >
          <BossTrack boss={boss} named={false} />
          {step === undefined ? null : (
            <span aria-hidden="true" className="text-xs font-semibold text-boss">
              {step.label}
            </span>
          )}
        </div>
      )}
      <ul
        ref={list}
        aria-label={`${name}'s abilities`}
        className="flex h-7 min-w-0 flex-1 flex-wrap items-center gap-1.5 overflow-hidden"
      >
        {abilities.map((ability, at) => (
          <li
            key={ability.key}
            className="flex h-7 min-w-0"
            data-clipped={at >= shown || undefined}
            aria-hidden={at >= shown || undefined}
          >
            <AbilityChip ability={ability} hidden={at >= shown} />
          </li>
        ))}
      </ul>
      <AllAbilities name={name} abilities={abilities} hidden={hidden} />
    </div>
  )
}

/** The abilities in full, for the intro screen of a fight: each with its kind, when it comes and its whole text. */
export function BossAbilityList({ abilities }: { abilities: readonly BossAbility[] }) {
  return (
    <ul aria-label="Boss abilities" className="grid gap-2 sm:grid-cols-2">
      {abilities.map((ability) => (
        <li
          key={ability.key}
          data-slot="boss-ability"
          className="grid gap-1 rounded-lg border bg-surface p-3"
        >
          <AbilityText ability={ability} title={false} />
        </li>
      ))}
    </ul>
  )
}

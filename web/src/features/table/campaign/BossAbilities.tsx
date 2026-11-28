/**
 * The boss's abilities across the top of a fight against a boss: one row of small chips, each its kind's icon and its
 * name, its card in the card preview on hover, focus or press. The row never wraps: chips that do not fit are left out
 * of it and counted on the "All" button at its end, which lists every ability in full. The phases stand in the top
 * bar's pill (`BossPhasePill`). The intro screen of a Delves fight lists the same abilities (`BossAbilityList`).
 */
import { CrownIcon, EyeIcon, FlameIcon, GemIcon, HourglassIcon, ListIcon, SparklesIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { useIsBlockerNamed } from '../blockedTell'
import { useSeatNamer } from '../names'
import { barOrder } from './bossAbilities'
import type { BossAbility, BossAbilityKind } from './bossAbilities'
import { KIND_WORDS, useBossAbilities } from './useBossAbilities'
import { useCustomPreview } from './useCustomPreview'

const ICONS: Record<BossAbilityKind, LucideIcon> = {
  innate: CrownIcon,
  phase: FlameIcon,
  'next-phase': HourglassIcon,
  modifier: SparklesIcon,
  relic: GemIcon,
  personality: EyeIcon,
}

/** Each kind's chip: the phase in force filled, what is always on plain, what is to come dashed. */
const CHIP: Record<BossAbilityKind, { chip: string; icon: string }> = {
  phase: { chip: 'border-boss bg-boss text-on-boss', icon: 'text-on-boss' },
  innate: { chip: 'border-boss/45 bg-card', icon: 'text-boss' },
  'next-phase': { chip: 'border-dashed border-boss/60 bg-card text-muted-foreground', icon: 'text-boss' },
  modifier: { chip: 'border-boss/45 bg-card', icon: 'text-boss' },
  relic: { chip: 'border-boss/45 bg-card', icon: 'text-boss' },
  personality: { chip: 'border-boss/45 bg-card', icon: 'text-boss' },
}

/** One ability's kind, when it comes, name and whole text: a row of the full list. */
function AbilityText({ ability }: { ability: BossAbility }) {
  const Icon = ICONS[ability.kind]
  return (
    <>
      <p className="flex items-center gap-1.5 text-xs font-medium text-boss">
        <Icon aria-hidden="true" className="size-3.5 shrink-0" />
        {KIND_WORDS[ability.kind]}
        {ability.when === undefined ? '' : ` · ${ability.when}`}
      </p>
      <p className="text-sm font-semibold">{ability.name}</p>
      <p data-slot="boss-ability-text" className="text-sm whitespace-pre-line text-muted-foreground">
        {ability.text === '' ? 'Its rules are being read…' : ability.text}
      </p>
    </>
  )
}

/**
 * One ability as a chip: its kind's icon and name, its summary for a screen reader; hover or focus shows its card in
 * the card preview and a press pins it there, as a card of the table does.
 */
function AbilityChip({
  ability,
  all,
  owner,
  hidden,
}: {
  ability: BossAbility
  all: readonly BossAbility[]
  owner: string
  hidden: boolean
}) {
  const Icon = ICONS[ability.kind]
  const tone = CHIP[ability.kind]
  // Its rule is what keeps the card the viewer just pressed from being played.
  const blocking = useIsBlockerNamed(ability.name)
  const handlers = useCustomPreview(ability, all, owner)
  return (
    <button
      type="button"
      {...handlers}
      tabIndex={hidden ? -1 : undefined}
      data-slot="boss-ability"
      data-kind={ability.kind}
      data-blocking={blocking || undefined}
      className={cn(
        'flex h-7 max-w-48 min-w-0 items-center gap-1 rounded-full border px-2 text-left text-xs font-semibold transition-colors outline-none hover:brightness-110 focus-visible:ring-2 focus-visible:ring-ring',
        tone.chip,
        blocking ? 'border-primary bg-primary/15 ring-2 ring-primary' : null,
      )}
    >
      <Icon aria-hidden="true" className={cn('size-3.5 shrink-0', tone.icon)} />
      <span className="truncate">{ability.name}</span>
      {ability.summary === '' ? null : <span className="sr-only"> {ability.summary}</span>}
    </button>
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

/** One row of the full list: the ability in full, a press showing its card in the card preview. */
function AbilityRow({
  ability,
  all,
  owner,
  onShow,
}: {
  ability: BossAbility
  all: readonly BossAbility[]
  owner: string
  onShow: () => void
}) {
  const handlers = useCustomPreview(ability, all, owner)
  return (
    <li data-slot="boss-ability-full">
      <button
        type="button"
        {...handlers}
        // The list closes as the card opens, so only a press shows it; hover and focus leave the list in view.
        onPointerEnter={undefined}
        onFocus={undefined}
        onBlur={undefined}
        onClick={(event) => {
          handlers?.onClick(event)
          onShow()
        }}
        className="grid w-full gap-0.5 px-3 py-2 text-left outline-none hover:bg-muted focus-visible:bg-muted"
      >
        <AbilityText ability={ability} />
      </button>
    </li>
  )
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
  const [open, setOpen] = useState(false)
  return (
    <Popover open={open} onOpenChange={setOpen}>
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
            <AbilityRow
              key={ability.key}
              ability={ability}
              all={abilities}
              owner={name}
              onShow={() => setOpen(false)}
            />
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  )
}

/**
 * The boss's abilities as one row of chips: its innate, personality, modifier and relics; what does not fit is counted on the button
 * at the end, which lists them all with the phases.
 */
export function BossAbilitiesBar({ seat }: { seat: PlayerId }) {
  const all = barOrder(useBossAbilities(seat))
  const name = useSeatNamer()(seat)
  // The phases stand in the top bar's pill; the row keeps the innate, personality, modifier and relics.
  const abilities = all.filter((ability) => ability.kind !== 'next-phase' && ability.kind !== 'phase')
  const list = useRef<HTMLUListElement>(null)
  const hidden = useWrapped(list, abilities.length)
  if (all.length === 0) return null
  const shown = abilities.length - hidden
  return (
    <div data-slot="boss-abilities" className="flex h-8 min-w-0 shrink-0 items-center gap-1.5 px-1">
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
            <AbilityChip ability={ability} all={all} owner={name} hidden={at >= shown} />
          </li>
        ))}
      </ul>
      <AllAbilities name={name} abilities={all} hidden={hidden} />
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
          <AbilityText ability={ability} />
        </li>
      ))}
    </ul>
  )
}

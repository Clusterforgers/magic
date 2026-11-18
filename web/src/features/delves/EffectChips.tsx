/** Effects as chips: gold, life, life cap, a relic (with its rarity), a card or a commander bane, each toned as a gain or a cost, and chances with their odds. */
import { CoinsIcon, GemIcon, HeartCrackIcon, HeartIcon, SkullIcon, SparklesIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { Chance, Chip, ChipKind, Part } from './eventEffects'
import { isEmpty } from './eventEffects'
import { RARITY_TONE } from './rarity'

const ICONS: Record<ChipKind, LucideIcon> = {
  gold: CoinsIcon,
  life: HeartIcon,
  cap: HeartCrackIcon,
  relic: GemIcon,
  card: SparklesIcon,
  bane: SkullIcon,
}

const TONES = {
  gain: 'border-success/60 text-success',
  cost: 'border-destructive/60 text-destructive',
  flat: 'border-border text-muted-foreground',
} as const

/** A chip's words; a relic's rarity word in its tier's colour. */
function ChipText({ chip }: { chip: Chip }) {
  const at = chip.rarity === undefined ? -1 : chip.text.indexOf(chip.rarity)
  if (chip.rarity === undefined || at < 0) return chip.text
  return (
    <>
      {chip.text.slice(0, at)}
      <span data-rarity={chip.rarity} className={RARITY_TONE[chip.rarity].text}>
        {chip.rarity}
      </span>
      {chip.text.slice(at + chip.rarity.length)}
    </>
  )
}

/** One effect: its icon and words, green for a gain, red for a cost. */
export function EffectChip({ chip }: { chip: Chip }) {
  const Icon = ICONS[chip.kind]
  return (
    <span
      data-chip={chip.kind}
      data-tone={chip.tone}
      className={cn(
        'inline-flex items-center gap-1 rounded-full border bg-surface px-2 py-1 text-sm font-semibold',
        TONES[chip.tone],
      )}
    >
      <Icon aria-hidden="true" className="size-3.5 shrink-0" />
      <span>
        <ChipText chip={chip} />
      </span>
    </span>
  )
}

/** What a branch of a chance gives: its chips and chances, "nothing" when empty. */
function Branch({ part }: { part: Part }) {
  if (isEmpty(part)) {
    return <span className="text-sm text-muted-foreground">nothing</span>
  }
  return <Effects part={part} />
}

function ChanceRows({ chance }: { chance: Chance }) {
  const rows = [
    { odds: chance.odds, part: chance.win, win: true },
    { odds: 100 - chance.odds, part: chance.lose, win: false },
  ]
  return (
    <span data-chance="" className="grid gap-1 rounded-lg border border-dashed px-2 py-2">
      {rows.map((row) => (
        <span key={row.win ? 'win' : 'lose'} className="flex flex-wrap items-center gap-2">
          <span className="min-w-10 text-sm font-semibold tabular-nums text-muted-foreground">
            {row.odds}%
          </span>
          <Branch part={row.part} />
        </span>
      ))}
    </span>
  )
}

/** Sure effects, then each chance with its odds. Made of spans only, so it sits inside a button. */
export function Effects({ part }: { part: Part }) {
  return (
    <span className="flex flex-wrap items-center gap-2">
      {part.chips.map((chip, i) => (
        <EffectChip key={`${chip.kind}-${i}`} chip={chip} />
      ))}
      {part.chances.map((chance, i) => (
        <ChanceRows key={`chance-${i}`} chance={chance} />
      ))}
    </span>
  )
}

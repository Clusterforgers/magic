/** The marks of a narrow compact face (`NARROW_PX`): an icon or a glyph of a few characters that reads whole, the words kept for a screen reader and the tooltip. */
import {
  BatteryLow,
  CheckCheck,
  CircleDot,
  Coins,
  Copy,
  EyeOff,
  Ghost,
  Heart,
  HeartCrack,
  Hourglass,
  Info,
  Layers,
  Paperclip,
  Repeat,
  RotateCw,
  ScanSearch,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Sparkles,
  Swords,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { CounterChip, StatusChip } from './cardModel'

/** A mark drawn short: an icon, a glyph of at most `GLYPH_MAX` characters, or both; `words` says it in full. */
export interface Glyph {
  icon: LucideIcon | null
  text: string
  words: string
}

/** The longest glyph text a narrow mark writes ("+99+" at most): it reads whole beside an icon on a 60 px face. */
export const GLYPH_MAX = 4

const STATUS_ICON: Partial<Record<string, LucideIcon>> = {
  Tapped: RotateCw,
  Sick: Hourglass,
  Attacking: Swords,
  Blocked: ShieldAlert,
  Blocking: Shield,
  'Face down': EyeOff,
  'Back face': Repeat,
  Copy: Copy,
  'Phased out': Ghost,
  Exerted: BatteryLow,
  Prepared: Sparkles,
  Suspected: ScanSearch,
  Solved: CheckCheck,
}

// A count past 99 writes "99+", so a glyph keeps to `GLYPH_MAX` characters.
function countText(count: number): string {
  return count > 99 ? '99+' : String(count)
}

/** A status chip as a glyph: its icon, plus the number of a damage, merged or level chip. */
export function statusGlyph(chip: StatusChip): Glyph {
  const words = chip.label === 'Sick' ? 'Summoning sick' : chip.label
  const damage = /^(\d+) dmg$/.exec(chip.label)
  if (damage !== null)
    return { icon: HeartCrack, text: countText(Number(damage[1])), words: `${damage[1]} damage` }
  const merged = /^Merged ×(\d+)$/.exec(chip.label)
  if (merged !== null) return { icon: Layers, text: countText(Number(merged[1])), words }
  const level = /^Level (\d+)$/.exec(chip.label)
  if (level !== null) return { icon: null, text: `L${countText(Number(level[1]))}`, words }
  return { icon: STATUS_ICON[chip.label] ?? CircleDot, text: '', words }
}

const COUNTER_ICON: Partial<Record<string, LucideIcon>> = { shield: ShieldCheck }

/** A counter chip as a glyph: "+2" for two +1/+1 counters, "−1" for a −1/−1, its symbol or icon and the count otherwise. */
export function counterGlyph(chip: CounterChip): Glyph {
  const words = `${chip.count} ${chip.key} ${chip.count === 1 ? 'counter' : 'counters'}`
  const count = countText(chip.count)
  if (chip.tone === 'up') return { icon: null, text: `+${count}`, words }
  if (chip.tone === 'down') return { icon: null, text: `−${count}`, words }
  if (chip.symbol !== chip.key) return { icon: null, text: `${chip.symbol}${count}`, words }
  return { icon: COUNTER_ICON[chip.key] ?? CircleDot, text: count, words }
}

/** A link of the face (what it is on, who protects it) as an icon; the names stay in its words. */
export function linkGlyph(kind: 'host' | 'protector', name: string): Glyph {
  return kind === 'host'
    ? { icon: Paperclip, text: '', words: `On ${name}` }
    : { icon: ShieldCheck, text: '', words: `Protected by ${name}` }
}

const MUST_ICON: Partial<Record<string, LucideIcon>> = {
  'Must attack': Swords,
  'Must block': Shield,
  'Must be blocked': ShieldAlert,
}

/**
 * The corner label a prompt writes on a card ("Pay {2}", "Needs 2 more", "At Ana") as a glyph: an icon, plus the cost,
 * the count or the label itself while it fits `GLYPH_MAX`; the label is its words.
 */
export function noteGlyph(label: string): Glyph {
  const short = (text: string): string => (text.length <= GLYPH_MAX ? text : '')
  const must = MUST_ICON[label]
  if (must !== undefined) return { icon: must, text: '!', words: label }
  const more = /^Needs (\d+) more$/.exec(label)
  if (more !== null) return { icon: Shield, text: `+${countText(Number(more[1]))}`, words: label }
  const least = /^Needs (\d+)\+ blockers$/.exec(label)
  if (least !== null) return { icon: Shield, text: `${countText(Number(least[1]))}+`, words: label }
  const life = /^Pay (\d+) life$/.exec(label)
  if (life !== null) return { icon: Heart, text: countText(Number(life[1])), words: label }
  const pay = /^Pay (.+)$/.exec(label)
  if (pay !== null) return { icon: Coins, text: short(pay[1] ?? ''), words: label }
  const text = short(label)
  return { icon: text === '' ? Info : null, text, words: label }
}

interface GlyphMarkProps {
  glyph: Glyph
  /** The mark's `data-slot`, as the wide face names it. */
  slot: string
  /** The look of the mark: the same background and border as its wide form. */
  className: string
  tone?: string | undefined
}

/** A narrow mark: never shrinks nor cuts, its words in a tooltip and for a screen reader. */
export function GlyphMark({ glyph, slot, className, tone }: GlyphMarkProps) {
  const Icon = glyph.icon
  return (
    <span
      data-slot={slot}
      data-tone={tone}
      data-glyph
      title={glyph.words}
      className={cn(
        'inline-flex h-4 shrink-0 items-center gap-px rounded-sm px-0.5 leading-none whitespace-nowrap tabular-nums',
        className,
      )}
    >
      {Icon === null ? null : <Icon aria-hidden="true" className="size-2.5 shrink-0" />}
      {glyph.text === '' ? null : <span aria-hidden="true">{glyph.text}</span>}
      <span className="sr-only">{glyph.words}</span>
    </span>
  )
}

import { Ban, Crown, Hand, Library, Play, Skull } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { seatHue, zoneLabel } from './podFigures'
import { AWAY_MARK } from './presenceText'
import type { ZoneKey } from './podFigures'

/** The icon of each zone chip. */
export const ZONE_ICON: Record<ZoneKey, LucideIcon> = {
  hand: Hand,
  library: Library,
  graveyard: Skull,
  exile: Ban,
  command: Crown,
}

/** A small dot in the colour of a seat, beside its name. Decoration: the name is always written next to it. */
export function SeatDot({ seat, className }: { seat: PlayerId; className?: string }) {
  return (
    <span
      aria-hidden="true"
      data-slot="seat-dot"
      style={{ '--seat-hue': seatHue(seat) } as CSSProperties}
      className={cn(
        'size-2.5 shrink-0 rounded-full bg-[oklch(0.55_0.16_var(--seat-hue))] dark:bg-[oklch(0.74_0.14_var(--seat-hue))]',
        className,
      )}
    />
  )
}

/**
 * The mark of a zone with a card in it that can be played or used now: a small disc with a play arrow on the corner of
 * the chip or picture, beside the ring of a legal card. Decoration: the chip's name says it in words.
 */
export function LegalBadge({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      data-slot="legal-badge"
      className={cn(
        'pointer-events-none absolute -top-1 -right-1 z-10 grid size-3.5 place-items-center rounded-full bg-primary text-primary-foreground',
        className,
      )}
    >
      <Play className="size-2 fill-current" />
    </span>
  )
}

/** The mark of a seat whose client is gone (`presenceText.isAway`), beside its name: a word, never colour alone. */
export function AwayMark({ className }: { className?: string }) {
  return (
    <span
      data-slot="away-mark"
      className={cn(
        'shrink-0 rounded border border-warning bg-warning/15 px-1 text-xs leading-4 font-semibold text-foreground',
        className,
      )}
    >
      {AWAY_MARK}
    </span>
  )
}

/** The ring a chip or picture with a card to play or use in it wears, as a legal card does (`HIGHLIGHT.legal`). */
export const LEGAL_RING = 'ring-2 ring-primary'

/** The 3 px edge of an opponent's pod or tile on its turn: the opponent's blue, inside the box so a column that clips its overflow keeps it. */
export const ACTIVE_EDGE = 'inset-ring-3 inset-ring-turn-them'

/** The faint tint of the active opponent's tile, in the colour of its edge; `cueTokens.test.ts` holds the text on it at 4.5:1. */
export const ACTIVE_TINT = 'bg-turn-them/5'

/** Your own pod on your turn: the gold glow of `your-turn-cue` with the 3 px gold edge drawn inside (`index.css`). */
export const YOUR_TURN_EDGE = 'your-turn-cue [--cue-edge:3px]'

/**
 * A dot beside the name of a seat that holds priority without the turn. It is an image with a name and a shape of
 * its own, so the cue is never colour alone; it takes the text colour, which every surface holds at 3:1 or more.
 */
export function PriorityDot({ label, className }: { label: string; className?: string }) {
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      data-slot="priority-dot"
      className={cn('size-2.5 shrink-0 rounded-full bg-foreground ring-2 ring-foreground/25', className)}
    />
  )
}

/** How roomy a chip is: the pod's panel, its one-line form, or the two-line tile of the strip. */
export type ChipDensity = 'panel' | 'line' | 'tile'

/** The size and shape every zone chip shares, the buttons among them. */
export const CHIP =
  'inline-flex min-h-6 min-w-0 items-center justify-center gap-1 rounded-lg text-xs font-medium pointer-coarse:min-h-8'

export const DENSITY: Record<ChipDensity, { chip: string; icon: string }> = {
  panel: { chip: cn(CHIP, 'bg-muted px-1.5'), icon: 'size-4' },
  line: { chip: cn(CHIP, 'gap-0.5 bg-muted px-0.5'), icon: 'size-3.5' },
  // A tile has no room for a chip's own background; its figures sit in a row of their own.
  tile: { chip: 'inline-flex shrink-0 items-center gap-0.5 text-xs font-medium', icon: 'size-3.5' },
}

interface CountChipProps {
  zone: ZoneKey
  count: number
  density?: ChipDensity
  className?: string
}

/** The number of cards in a zone nobody opens: an icon and the count, named in words ("Hand 7 cards"). */
export function CountChip({ zone, count, density = 'panel', className }: CountChipProps) {
  const Icon = ZONE_ICON[zone]
  const label = zoneLabel(zone, count)
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      data-zone={zone}
      className={cn(DENSITY[density].chip, 'text-muted-foreground', className)}
    >
      <Icon aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon)} />
      <span aria-hidden="true" className="tabular-nums">
        {count}
      </span>
    </span>
  )
}

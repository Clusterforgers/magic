import { Ban, Crown, Hand, Library, Skull } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { CSSProperties } from 'react'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { seatHue, zoneLabel } from './podFigures'
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

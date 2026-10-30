import { Link2 } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { EnchantChip as Chip } from './enchantChips'
import { CHIP } from './PodParts'
import type { ChipDensity } from './PodParts'
import { usePreviewHandlers } from './usePreviewHandlers'

const SHAPE: Record<ChipDensity, string> = {
  panel: cn(CHIP, 'bg-muted px-1.5 text-muted-foreground'),
  line: cn(CHIP, 'gap-0.5 bg-muted px-1 text-muted-foreground'),
  tile: 'inline-flex shrink-0 items-center gap-0.5 rounded px-0.5 text-xs font-medium',
}

function Mark({ chip, density }: { chip: Chip; density: ChipDensity }) {
  return (
    <>
      <Link2 aria-hidden="true" className="size-3.5 shrink-0" />
      <span aria-hidden="true" className="truncate tabular-nums">
        {density === 'tile' && chip.count > 1 ? `×${chip.count}` : chip.text}
      </span>
    </>
  )
}

/** In a pod the chip is a button: focus or hover previews the Aura, a press pins the preview. */
function ChipButton({ chip, density }: { chip: Chip; density: ChipDensity }) {
  const handlers = usePreviewHandlers(chip.key)
  return (
    <button
      type="button"
      aria-label={chip.words}
      title={chip.words}
      data-slot="enchant-chip"
      data-count={chip.count}
      {...handlers}
      className={cn(SHAPE[density], 'min-w-0 outline-offset-2 outline-primary focus-visible:outline-2')}
    >
      <Mark chip={chip} density={density} />
    </button>
  )
}

/**
 * One chip for each kind of Aura or Curse attached to the player (docs/ui-design.md, Attachments), with a count when
 * several share a name. In a pod each is a button that previews the Aura; in a tile, which is a button itself, each
 * is an image and the tile's spoken name carries the same words.
 */
export function EnchantChips({ chips, density }: { chips: readonly Chip[]; density: ChipDensity }) {
  return chips.map((chip) =>
    density === 'tile' ? (
      <span
        key={chip.key}
        role="img"
        aria-label={chip.words}
        title={chip.words}
        data-slot="enchant-chip"
        data-count={chip.count}
        className={cn(SHAPE.tile, 'min-w-0 text-muted-foreground')}
      >
        <Mark chip={chip} density={density} />
      </span>
    ) : (
      <ChipButton key={chip.key} chip={chip} density={density} />
    ),
  )
}

import { cn } from '@/lib/utils'
import type { Color } from '@/protocol/generated'

// Whole class names, so Tailwind can see them.
const FILL: Record<Color, string> = {
  White: 'bg-mana-white',
  Blue: 'bg-mana-blue',
  Black: 'bg-mana-black',
  Red: 'bg-mana-red',
  Green: 'bg-mana-green',
}

/** A small dot in a Magic colour. It is decoration: always print the colour's name beside it. */
export function ColorDot({ color, className }: { color: Color; className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'inline-block size-2.5 shrink-0 rounded-full ring-1 ring-foreground/30',
        FILL[color],
        className,
      )}
    />
  )
}

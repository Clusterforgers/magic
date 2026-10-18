import { cn } from '@/lib/utils'
import { SIZE } from '../cardSize'
import type { CardSize } from '../cardSize'
import { CardFace } from '../CardFace'
import { FxFloat } from './FxFloat'
import type { Ghost } from './model'

/**
 * A permanent that has just died, drawn where it stood while its effect plays: the stylesheet cracks it, greys it
 * out and closes the gap it leaves. It is `aria-hidden`, has no `data-object-id` and nothing focusable, so it is
 * never a second copy of the card for a screen reader, the keyboard or a script that looks cards up. The real card
 * is already in the graveyard, which is where the game put it.
 */
export function GhostCard({ ghost, size }: { ghost: Ghost; size: CardSize }) {
  const { object } = ghost
  const tapped = object.permanent?.tapped === true
  return (
    <li
      aria-hidden="true"
      data-ghost={object.id}
      data-fx="died"
      className={cn(
        'flex w-(--ghost-w) shrink-0 overflow-hidden',
        SIZE[size].vars,
        tapped ? '[--ghost-w:var(--card-h)]' : '[--ghost-w:var(--card-w)]',
      )}
    >
      <span className="relative grid h-(--card-h) w-full place-items-center">
        <CardFace object={object} role={null} size={size} />
        <svg
          aria-hidden="true"
          focusable="false"
          viewBox="0 0 60 84"
          preserveAspectRatio="none"
          className="fx-crack pointer-events-none absolute inset-0 size-full"
        >
          <path pathLength={1} d="M31 0 L25 16 L35 28 L22 44 L33 58 L27 70 L31 84" />
          <path pathLength={1} d="M35 28 L52 34 M22 44 L6 50" />
        </svg>
        <FxFloat kind="damaged" amount={ghost.damage} />
      </span>
    </li>
  )
}

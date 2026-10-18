import { memo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { cardLabel } from './cardModel'
import type { Highlight } from './cardModel'
import { HIGHLIGHT, SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { CardFace } from './CardFace'
import { combatRoleIn } from './selectors'
import { usePreviewHandlers } from './usePreviewHandlers'

/** Props of a card. */
export interface CardProps {
  /** The object to draw; the card reads it from the store itself, so a parent passes only the id. */
  id: ObjectId
  size?: CardSize
  /** False takes the card out of the tab order; a row keeps exactly one of its cards tabbable. */
  tabbable?: boolean
  /** Seam for the decision layer: marks the card as a legal choice, a dimmed one or the chosen one. */
  highlight?: Highlight
  /** Seam for the decision layer: called with the id when the card is clicked or activated by keyboard. */
  onSelect?: (id: ObjectId) => void
}

/**
 * One card of the table. It subscribes to its own object and combat role only and is memoised, so a batch redraws
 * the cards it changed and no others. Hover and focus preview it; a click pins the preview and calls `onSelect`.
 */
export const Card = memo(function Card({
  id,
  size = 'normal',
  tabbable = true,
  highlight,
  onSelect,
}: CardProps) {
  const object = usePlay((state) => state.objectsById[id])
  const role = usePlay((state) => combatRoleIn(state, id))
  const handlers = usePreviewHandlers(id, onSelect === undefined ? undefined : () => onSelect(id))
  if (object === undefined) return null
  const tapped = object.permanent?.tapped === true
  return (
    <button
      type="button"
      data-object-id={id}
      data-highlight={highlight}
      data-size={size}
      data-tapped={tapped || undefined}
      aria-label={cardLabel(object, role)}
      aria-pressed={highlight === undefined ? undefined : highlight === 'selected'}
      tabIndex={tabbable ? 0 : -1}
      {...handlers}
      className={cn(
        'grid h-(--card-h) shrink-0 place-items-center rounded-md outline-offset-2 outline-primary focus-visible:outline-2',
        SIZE[size].vars,
        tapped ? 'w-(--card-h)' : 'w-(--card-w)',
        highlight === undefined ? null : HIGHLIGHT[highlight],
      )}
    >
      <CardFace object={object} role={role} size={size} />
    </button>
  )
})

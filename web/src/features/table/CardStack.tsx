import { memo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { cardLabel } from './cardModel'
import { HIGHLIGHT, SIZE } from './cardSize'
import type { CardSize } from './cardSize'
import { CardFace } from './CardFace'
import { useCardMark } from './prompt/marks'
import { usePreviewHandlers } from './usePreviewHandlers'

/** The "×N" chip of a stack, at the foot of its first card, where the face has the least to say. */
export function StackCount({ count, className }: { count: number; className?: string }) {
  return (
    <span
      aria-hidden="true"
      data-slot="stack-count"
      className={cn(
        'absolute -bottom-1 -left-1 rounded-full bg-foreground px-1.5 text-[0.6875rem] leading-4 font-semibold text-background tabular-nums',
        className,
      )}
    >
      ×{count}
    </span>
  )
}

/** Props of a closed stack. */
export interface CardStackProps {
  /** The permanent drawn on top of the pile: the stack's first member. */
  id: ObjectId
  /** How many equal permanents the pile holds. */
  count: number
  size?: CardSize
  /** The box comes from the row the stack sits in, as for a `Card`. */
  fit?: boolean
  /** False takes the stack out of the tab order; a row keeps exactly one of its cards tabbable. */
  tabbable?: boolean
  /** Opens the stack in place, given the card on top; called by a click and by Enter or Space. */
  onOpen: (id: ObjectId) => void
}

/**
 * Equal permanents drawn as one card with a "×N" chip and two edges behind it. A click or Enter opens it in place
 * (`onOpen`); hover and focus preview the card on top. It reads only that card's object, and is memoised.
 */
export const CardStack = memo(function CardStack({
  id,
  count,
  size = 'normal',
  fit = false,
  tabbable = true,
  onOpen,
}: CardStackProps) {
  const object = usePlay((state) => state.objectsById[id])
  // A stack with a member the prompt needs is open, so the only mark a closed one carries is the dimming of the rest.
  const { highlight } = useCardMark(id)
  // A click opens the stack and pins nothing: it is an act, not a request to look.
  const handlers = usePreviewHandlers(id, () => onOpen(id))
  if (object === undefined) return null
  const tapped = object.permanent?.tapped === true
  return (
    <button
      type="button"
      data-object-id={id}
      data-stack={count}
      data-highlight={highlight}
      data-size={size}
      data-tapped={tapped || undefined}
      aria-label={`${count} × ${cardLabel(object, null)}, stacked`}
      aria-expanded={false}
      tabIndex={tabbable ? 0 : -1}
      {...handlers}
      className={cn(
        'relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-md outline-offset-2 outline-primary focus-visible:outline-2',
        fit ? null : SIZE[size].vars,
        highlight === undefined ? null : HIGHLIGHT[highlight],
      )}
    >
      <CardFace object={object} role={null} size={size} fit={fit} stacked />
      <StackCount count={count} />
    </button>
  )
})

import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { CardFace } from '../CardFace'
import { PROMPT_CARD_BOX } from './PromptCard'
import type { PromptCardSize } from './PromptCard'

/**
 * A card of a prompt drawn as a picture and nothing to press: the card face from the view, or a face-down stand-in
 * for a card the view does not hold. Decoration (`aria-hidden`): whatever holds it names the card in words.
 */
export function PromptPicture({
  object,
  size,
  className,
}: {
  object: ObjectView | undefined
  size: PromptCardSize
  className?: string
}) {
  return (
    <span
      aria-hidden="true"
      data-slot="prompt-picture"
      className={cn(
        'relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center',
        PROMPT_CARD_BOX[size],
        className,
      )}
    >
      {object === undefined ? (
        <span
          data-slot="card-face"
          data-hidden=""
          className="grid h-(--card-h) w-(--card-w) place-items-center rounded-md border-2 border-border-strong bg-muted text-xs text-muted-foreground"
        >
          Card
        </span>
      ) : (
        <CardFace object={object} role={null} size={size} fit />
      )}
    </span>
  )
}

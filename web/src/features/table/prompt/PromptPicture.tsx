import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { CardFace } from '../CardFace'
import type { PromptCardSize } from './PromptCard'

/** The box of each size, 5:7 like a card; whole class names, so Tailwind can see them. */
const BOX: Record<PromptCardSize, string> = {
  normal: '[--card-w:4.75rem] [--card-h:6.65rem]',
  compact: '[--card-w:3.75rem] [--card-h:5.25rem]',
}

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
        BOX[size],
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

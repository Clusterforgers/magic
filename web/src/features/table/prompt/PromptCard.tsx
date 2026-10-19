import { Check } from 'lucide-react'
import type { Ref } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { HIGHLIGHT } from '../cardSize'
import { CardFace } from '../CardFace'
import { usePreviewHandlers } from '../usePreviewHandlers'

/** How a card of a prompt is drawn: whole (cost and type line, from 76 px wide) or compact (name and power only). */
export type PromptCardSize = 'normal' | 'compact'

/** The box of each size, 5:7 like a card; whole class names, so Tailwind can see them. */
const BOX: Record<PromptCardSize, string> = {
  normal: '[--card-w:4.75rem] [--card-h:6.65rem]',
  compact: '[--card-w:3.75rem] [--card-h:5.25rem]',
}

/** Props of `PromptCard`. */
export interface PromptCardProps {
  id: ObjectId
  /** What a screen reader calls the card; two cards of one name are told apart by the caller. */
  label: string
  size: PromptCardSize
  /** The card is in the choice. */
  chosen: boolean
  /** The choice is full and this card is not in it. */
  closed: boolean
  /** The one card of its list that Tab reaches. */
  tabbable: boolean
  /** The key that toggles the card, if it has one. */
  hotkey?: string | undefined
  /** A click or activation on a card that is not closed. */
  onPress(id: ObjectId): void
  ref?: Ref<HTMLButtonElement>
}

/**
 * A card offered by a prompt, drawn from the view like a card of the table but as a toggle of its own: it carries
 * `data-candidate` (never `data-object-id`, which belongs to the table's card the same object is) and shows its
 * choice by a check and a ring, never by colour alone. A card the view does not hold is a face-down stand-in.
 */
export function PromptCard({
  id,
  label,
  size,
  chosen,
  closed,
  tabbable,
  hotkey,
  onPress,
  ref,
}: PromptCardProps) {
  const object = usePlay((state) => state.objectsById[id])
  const handlers = usePreviewHandlers(id, () => {
    if (!closed) onPress(id)
  })
  return (
    <button
      ref={ref}
      type="button"
      data-candidate={id}
      data-chosen={chosen || undefined}
      data-size={size}
      aria-label={label}
      aria-pressed={chosen}
      aria-disabled={closed || undefined}
      aria-keyshortcuts={hotkey}
      tabIndex={tabbable ? 0 : -1}
      {...handlers}
      className={cn(
        'relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-md outline-offset-2 outline-primary focus-visible:outline-2',
        BOX[size],
        chosen
          ? HIGHLIGHT.selected
          : closed
            ? HIGHLIGHT.dimmed
            : 'ring-1 ring-border-strong hover:ring-2 hover:ring-primary',
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
      {chosen ? (
        <span
          aria-hidden="true"
          data-slot="chosen-mark"
          className="absolute -top-1.5 -right-1.5 grid size-5 place-items-center rounded-full bg-primary text-primary-foreground ring-2 ring-background"
        >
          <Check className="size-3.5" />
        </span>
      ) : null}
      {hotkey === undefined ? null : (
        <kbd
          aria-hidden="true"
          data-slot="key-hint"
          className="absolute bottom-1 left-1 inline-flex min-w-5 items-center justify-center rounded border border-border-strong bg-background px-1 font-mono text-xs text-foreground"
        >
          {hotkey}
        </kbd>
      )}
    </button>
  )
}

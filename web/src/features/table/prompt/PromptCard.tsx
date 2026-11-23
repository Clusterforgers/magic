import { Crosshair } from 'lucide-react'
import type { Ref } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { HIGHLIGHT } from '../cardSize'
import { CardFace } from '../CardFace'
import { ChosenMark } from '../ChosenMark'
import { usePreviewHandlers } from '../usePreviewHandlers'
import { useChoiceAnchor } from './choiceAnchor'

/** How a card of a prompt is drawn: whole (cost and type line, from 76 px wide) or compact (name and power only). */
export type PromptCardSize = 'normal' | 'compact'

/**
 * The box of each size, 5:7 like a card: a whole card up to 7 rem wide on a tall window, and no taller than the room of
 * its grid (`--choice-room`), so a row is always whole; in the band's row of cards the width it sets (`--choice-fit`,
 * `stripCardWidth`); whole class names, so Tailwind can see them.
 */
export const PROMPT_CARD_BOX: Record<PromptCardSize, string> = {
  normal:
    '[--card-h:calc(var(--card-w)*7/5)] [--card-w:var(--choice-fit,max(3rem,min(7rem,11dvh,calc((var(--choice-room,20rem)_-_2rem)*5/7))))] max-sm:[--card-w:4.75rem]',
  compact:
    '[--card-h:calc(var(--card-w)*7/5)] [--card-w:max(3rem,min(3.75rem,calc((var(--choice-room,20rem)_-_2rem)*5/7)))]',
}

/** Up to this many cards on offer are drawn whole; a longer list is compact, so a row holds more cards. */
export const NORMAL_UP_TO = 12

/**
 * The box of a card in the long-list overlay: 5:7 at the width the panel's layout picked (`--overlay-card-w`, set on the
 * grid by `CardChoices`), whole at any size the layout allows.
 */
const OVERLAY_CARD_BOX = '[--card-h:calc(var(--card-w)*7/5)] [--card-w:var(--overlay-card-w,5.5rem)]'

/** The size for a list of `count` cards. */
export function tileSizeFor(count: number): PromptCardSize {
  return count <= NORMAL_UP_TO ? 'normal' : 'compact'
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
  /** The card stands in the long-list overlay, at the width its grid sets (`OVERLAY_CARD_BOX`). */
  overlay?: boolean
  /** A short word on the card about this choice ("Targeted"), drawn on its top edge and read after its name. */
  badge?: string | undefined
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
  overlay = false,
  badge,
  ref,
}: PromptCardProps) {
  const object = usePlay((state) => state.objectsById[id])
  const handlers = usePreviewHandlers(
    id,
    () => {
      if (!closed) onPress(id)
    },
    useChoiceAnchor(),
  )
  return (
    <button
      ref={ref}
      type="button"
      data-candidate={id}
      data-chosen={chosen || undefined}
      data-size={size}
      aria-label={badge === undefined ? label : `${label}, ${badge.toLowerCase()}`}
      aria-pressed={chosen}
      aria-disabled={closed || undefined}
      aria-keyshortcuts={hotkey}
      tabIndex={tabbable ? 0 : -1}
      {...handlers}
      className={cn(
        'relative grid h-(--card-h) w-(--card-w) shrink-0 place-items-center rounded-md outline-offset-2 outline-primary focus-visible:outline-2',
        overlay ? OVERLAY_CARD_BOX : PROMPT_CARD_BOX[size],
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
        // Every card of a choice stands upright and whole, tapped or not, so the row reads as one size.
        <CardFace object={object} role={null} size={size} fit upright />
      )}
      {badge === undefined ? null : (
        <span
          aria-hidden="true"
          data-slot="card-badge"
          className="absolute top-1 left-1/2 z-10 inline-flex max-w-[90%] -translate-x-1/2 items-center gap-1 truncate rounded-sm bg-warning px-1 text-xs leading-snug font-medium text-warning-foreground shadow-sm"
        >
          <Crosshair className="size-3 shrink-0" />
          {badge}
        </span>
      )}
      {chosen ? <ChosenMark /> : null}
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

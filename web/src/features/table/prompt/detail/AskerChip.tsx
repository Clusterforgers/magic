import { useState } from 'react'
import { cardImageUrl, imageSourceOf } from '@/features/cards/images'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { usePreviewHandlers } from '../../usePreviewHandlers'
import { lightHandlers } from './boardBadges'
import { useAsker } from './useAsker'

/** A card's art in a disc; the first letter of its name where it has no picture. */
export function ArtDisc({
  object,
  name,
  className,
}: {
  object: ObjectView | undefined
  name: string
  className?: string
}) {
  const [missed, setMissed] = useState(false)
  const source = object === undefined ? null : imageSourceOf(object)
  return (
    <span
      aria-hidden="true"
      data-slot="art-disc"
      className={cn(
        'grid size-9 shrink-0 place-items-center overflow-hidden rounded-full bg-muted text-sm font-bold text-muted-foreground ring-1 ring-border-strong',
        className,
      )}
    >
      {source === null || missed ? (
        (name.trim()[0] ?? '?').toUpperCase()
      ) : (
        <img
          src={cardImageUrl(source, 0, 'art_crop')}
          alt=""
          decoding="async"
          onError={() => setMissed(true)}
          className="size-full object-cover"
        />
      )}
    </span>
  )
}

/**
 * The asking card at the band's left end: its art and its name. Hover, focus or a press lights the card on the board and
 * shows its preview; a click pins the preview. Nothing for a decision no card asks.
 */
export function AskerChip() {
  const asker = useAsker()
  if (asker === null || asker.shown === '') return null
  return <Chip id={asker.object?.id} board={asker.board} object={asker.object} name={asker.shown} />
}

/** A chip at the band's left for a decision no one card asks: `object`'s art beside `words` ("Order 4 triggers"). */
export function LeadChip({ object, name, words }: { object: ObjectView; name: string; words: string }) {
  return <Chip id={object.id} board={object.id} object={object} name={name} words={words} />
}

function Chip({
  id,
  board,
  object,
  name,
  words,
}: {
  id: number | undefined
  board: number | undefined
  object: ObjectView | undefined
  name: string
  /** What the chip says in place of the name. */
  words?: string
}) {
  // An id the view does not hold previews nothing; -1 is no object.
  const preview = usePreviewHandlers(id ?? -1)
  const light = lightHandlers(board)
  const both =
    id === undefined
      ? light
      : {
          onClick: preview.onClick,
          onPointerEnter: (event: Parameters<typeof preview.onPointerEnter>[0]) => {
            light.onPointerEnter?.()
            preview.onPointerEnter(event)
          },
          onPointerLeave: () => {
            light.onPointerLeave?.()
            preview.onPointerLeave()
          },
          onPointerDown: light.onPointerDown,
          onPointerUp: light.onPointerUp,
          onFocus: (event: Parameters<typeof preview.onFocus>[0]) => {
            light.onFocus?.()
            preview.onFocus(event)
          },
          onBlur: () => {
            light.onBlur?.()
            preview.onBlur()
          },
        }
  return (
    <button
      type="button"
      data-slot="prompt-asker"
      data-asker={board ?? id}
      aria-label={
        words === undefined ? `${name} asks. Show the card.` : `${words}, from ${name}. Show the card.`
      }
      title={name}
      className="asker-chip outline-primary focus-visible:outline-2"
      {...both}
    >
      <ArtDisc object={object} name={name} />
      <span className="truncate">{words ?? name}</span>
    </button>
  )
}

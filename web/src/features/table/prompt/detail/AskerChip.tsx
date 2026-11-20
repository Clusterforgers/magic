import { useState } from 'react'
import { cardImageUrl, imageSourceOf } from '@/features/cards/images'
import { cn } from '@/lib/utils'
import type { ObjectView } from '@/protocol/generated'
import { usePreviewHandlers } from '../../usePreviewHandlers'
import { lightHandlers } from './boardBadges'
import { useAsker } from './useAsker'
import { anchor } from '@/features/learn/anchors'

/** The coach's anchor of the asking card: its chip in the dock or bar, its entry in the Stack panel; one is drawn. */
export const ASK_CARD = anchor('ask.card')

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
export function AskerChip({ doing }: { doing?: string | undefined } = {}) {
  const asker = useAsker()
  if (asker === null || asker.shown === '') return null
  return (
    <Chip id={asker.object?.id} board={asker.board} object={asker.object} name={asker.shown} doing={doing} />
  )
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
  doing,
}: {
  id: number | undefined
  board: number | undefined
  object: ObjectView | undefined
  /** What is being done with the card, drawn over its name ("Casting", "Plainscycling"). */
  doing?: string | undefined
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
      {...ASK_CARD}
      data-asker={board ?? id}
      aria-label={
        doing !== undefined
          ? `${doing} ${name}. Show the card.`
          : words === undefined
            ? `${name} asks. Show the card.`
            : `${words}, from ${name}. Show the card.`
      }
      title={doing === undefined ? name : `${doing} ${name}`}
      className="asker-chip outline-primary focus-visible:outline-2"
      {...both}
    >
      <ArtDisc object={object} name={name} />
      {doing === undefined ? (
        <span className="truncate">{words ?? name}</span>
      ) : (
        <span className="grid min-w-0">
          <span data-slot="asker-doing" className="truncate text-xs leading-4 font-semibold text-primary">
            {doing}
          </span>
          <span className="truncate text-[0.8125rem] leading-4">{name}</span>
        </span>
      )}
    </button>
  )
}

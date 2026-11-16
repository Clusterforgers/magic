import type { CSSProperties } from 'react'
import type { ObjectView } from '@/protocol/generated'
import { CardFace } from '../CardFace'
import { CardWidthContext } from '../ImageFace'
import { usePreviewHandlers } from '../usePreviewHandlers'

/** From this picture width (px) a revealed card's face is drawn at the `medium` size, below it at `normal`. */
const MEDIUM_FROM_PX = 128

/**
 * One card of a reveal, as compact as a card of the table: its picture at `width` and its name on one line under it,
 * cut with an ellipsis. Its rules text is not drawn here: hovering, focusing or clicking the tile shows the card's
 * preview, which holds the whole text and keeps its own place on the screen, so a reveal of many cards stays small.
 * The preview is given the card as it was shown, since the view may hold it no longer.
 */
export function RevealTile({ card, width }: { card: ObjectView; width: number }) {
  const handlers = usePreviewHandlers(card.id, undefined, undefined, card)
  const box = { '--card-w': `${width}px`, '--card-h': `${(width * 7) / 5}px` } as CSSProperties
  return (
    <li data-slot="moment-card" className="grid shrink-0 gap-1" style={{ width }}>
      <button
        type="button"
        data-object-id={card.id}
        aria-label={`${card.name}, revealed`}
        style={box}
        className="relative grid h-(--card-h) w-(--card-w) place-items-center rounded-md outline-offset-2 outline-primary focus-visible:outline-2"
        {...handlers}
      >
        <CardWidthContext.Provider value={width}>
          <CardFace
            object={card}
            role={null}
            size={width >= MEDIUM_FROM_PX ? 'medium' : 'normal'}
            fit
            upright
          />
        </CardWidthContext.Provider>
      </button>
      <span
        data-slot="moment-card-name"
        title={card.name}
        aria-hidden="true"
        className="block truncate text-center text-xs"
      >
        {card.name}
      </span>
    </li>
  )
}

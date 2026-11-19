/** The opening hand drawn large in the prompt while its keep or mulligan is decided, as Arena shows it. */
import { anchor } from '@/features/learn/anchors'
import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayState } from '@/play/store'
import type { ObjectId } from '@/protocol/generated'
import { CardFace } from '../CardFace'
import { usePreviewHandlers } from '../usePreviewHandlers'

const NONE: readonly ObjectId[] = []

/** The viewer's hand in order, or none for a viewer without a seat or a hand it cannot see; wrap in `useShallow`. */
export function openingHandIds(state: PlayState): readonly ObjectId[] {
  if (state.seat === null) return NONE
  return state.view?.players.find((player) => player.seat === state.seat)?.hand.cards ?? NONE
}

function OpeningCard({ id }: { id: ObjectId }) {
  const object = usePlay((state) => state.objectsById[id])
  const handlers = usePreviewHandlers(id)
  if (object === undefined) return null
  return (
    <li
      data-slot="opening-card"
      className="grid h-(--card-h) w-(--card-w) shrink-0 place-items-center"
      {...handlers}
    >
      <CardFace object={object} role={null} size="normal" fit />
    </li>
  )
}

/**
 * The cards of the hand being judged, whole and side by side up to 7 rem wide, so the keep is decided on the
 * cards, not on their names in the dock: seven fill the row, never below 3 rem, and a row wider than the band scrolls (a phone) rather than wrap under the answers. Decoration for the eye: the hand dock holds the same cards for a screen
 * reader and the keyboard, so the row is `aria-hidden` apart from its preview on hover.
 */
export function OpeningHand({ card }: { card?: number } = {}) {
  const coach = anchor('opening.hand')
  const ids = usePlay(useShallow(openingHandIds))
  if (ids.length === 0) return null
  if (card !== undefined) {
    // The floating panel: one row of cards at the width the room gives (`openingCardPx`).
    return (
      <ul
        aria-hidden="true"
        data-slot="opening-hand"
        {...coach}
        {...coach}
        data-large=""
        style={{ '--card-w': `${card}px`, '--card-h': `${Math.round((card * 7) / 5)}px` } as CSSProperties}
        className="flex basis-full justify-center gap-2"
      >
        {ids.map((id) => (
          <OpeningCard key={id} id={id} />
        ))}
      </ul>
    )
  }
  return (
    <ul
      aria-hidden="true"
      data-slot="opening-hand"
      {...coach}
      className="flex basis-full justify-center-safe gap-2 overflow-x-auto [container-type:inline-size] [--card-h:calc(var(--card-w)*7/5)] [--card-w:max(3rem,min(7rem,11dvh,calc((100cqw_-_3rem)/7)))]"
    >
      {ids.map((id) => (
        <OpeningCard key={id} id={id} />
      ))}
    </ul>
  )
}

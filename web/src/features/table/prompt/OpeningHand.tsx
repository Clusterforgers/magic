/** The opening hand drawn large in the prompt while its keep or mulligan is decided, as Arena shows it. */
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
 * cards, not on their names in the dock: seven fill the row, never below 3 rem, wrapping only then. Decoration for the eye: the hand dock holds the same cards for a screen
 * reader and the keyboard, so the row is `aria-hidden` apart from its preview on hover.
 */
export function OpeningHand() {
  const ids = usePlay(useShallow(openingHandIds))
  if (ids.length === 0) return null
  return (
    <ul
      aria-hidden="true"
      data-slot="opening-hand"
      className="flex basis-full flex-wrap justify-center gap-2 [container-type:inline-size] [--card-h:calc(var(--card-w)*7/5)] [--card-w:max(3rem,min(7rem,11dvh,calc((100cqw_-_3rem)/7)))]"
    >
      {ids.map((id) => (
        <OpeningCard key={id} id={id} />
      ))}
    </ul>
  )
}

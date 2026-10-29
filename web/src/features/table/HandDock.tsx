import { memo, useMemo, useRef } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, ObjectView, PlayerId, PlayerView } from '@/protocol/generated'
import { castCostLine } from './commander'
import { HandBacks } from './Hand'
import { handLayout } from './handFit'
import { HandFan } from './HandFan'
import type { HandCommander } from './HandFan'
import { useSeatNamer } from './names'
import { playerOf } from './selectors'
import { useCoarsePointer } from './useCoarsePointer'
import { useContentBox } from './useZoneFit'

/** Room kept round the line, so the focus ring of the first and last card is not cut by the line's scrolling. */
const PAD_PX = 4

/** A card narrower than this draws the compact face (name and cost); from here it draws the type line and the rest too. */
const NORMAL_FACE_PX = 76

const NO_COMMANDERS: ReadonlyMap<ObjectId, HandCommander> = new Map()

/**
 * The seat's commanders that wait in its command zone, in command zone order, each with its tax and the words of
 * where it is and what casting it costs ("in the command zone, costs {3}{G}{G} + {2} tax").
 */
export function commandersInHand(
  player: PlayerView | undefined,
  objects: Readonly<Record<ObjectId, ObjectView>>,
): ReadonlyMap<ObjectId, HandCommander> {
  const commanders = player?.commanders ?? []
  const command = player?.command.cards ?? []
  if (commanders.length === 0 || command.length === 0) return NO_COMMANDERS
  const waiting = new Map<ObjectId, HandCommander>()
  for (const id of command) {
    const commander = commanders.find((entry) => entry.object === id)
    if (commander === undefined) continue
    const cost = castCostLine(objects[id]?.mana_cost ?? null, commander.tax)
    waiting.set(id, {
      tax: commander.tax,
      words: cost === '' ? 'in the command zone' : `in the command zone, costs ${cost}`,
    })
  }
  return waiting.size === 0 ? NO_COMMANDERS : waiting
}

/**
 * The hand dock, the bottom edge of the table: the viewer's cards as a fan as tall as the dock allows, side by side
 * while they fit and overlapping just enough when they do not (at least 62 % of a card shows), a hovered or focused card
 * rising by 40 % of its height and opening the preview. The cards are the same buttons as everywhere on the table, so a
 * click plays or casts them as the active prompt says. The fan stays inside the dock's width, so it never reaches the
 * pass controls beside it. The commanders waiting in the command zone lead the fan with a crown and the tax on their cast,
 * so a commander is cast from here like any card of the hand.
 */
export const HandDock = memo(function HandDock({ seat }: { seat: PlayerId }) {
  const owner = useSeatNamer()(seat)
  const player = usePlay((state) => playerOf(state, seat))
  const objects = usePlay((state) => state.objectsById)
  const hand = player?.hand.cards ?? null
  const waiting = useMemo(() => commandersInHand(player, objects), [player, objects])
  const cards = useMemo(
    () => (hand === null ? null : waiting.size === 0 ? hand : [...waiting.keys(), ...hand]),
    [hand, waiting],
  )
  const count = player?.hand.count ?? 0
  const zone = useRef<HTMLDivElement>(null)
  const room = useContentBox(zone)
  const coarse = useCoarsePointer()
  const layout = useMemo(() => {
    if (room === null || cards === null) return undefined
    return handLayout({
      width: room.width - 2 * PAD_PX,
      height: room.height - 2 * PAD_PX,
      count: cards.length,
      coarse,
    })
  }, [room, cards, coarse])
  return (
    <section
      aria-label="Your hand"
      data-region="hand"
      className="flex min-h-0 min-w-0 items-center justify-center border-t bg-table px-1 py-1"
    >
      <div ref={zone} className="flex size-full min-h-0 min-w-0 items-end justify-center">
        {cards === null ? (
          <HandBacks count={count} />
        ) : cards.length === 0 ? (
          <p className="self-center text-sm text-muted-foreground">Your hand is empty.</p>
        ) : (
          <HandFan
            ids={cards}
            label={`${owner}'s hand`}
            size={layout !== undefined && layout.box.width < NORMAL_FACE_PX ? 'compact' : 'normal'}
            cost
            layout={layout}
            commanders={waiting}
          />
        )}
      </div>
    </section>
  )
})

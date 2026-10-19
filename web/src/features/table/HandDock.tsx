import { memo, useMemo, useRef } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import { CARD_ASPECT } from './fit'
import { HandBacks } from './Hand'
import { handCardWidth, handFan } from './handFit'
import { CardRow } from './CardRow'
import type { CardBox } from './CardRow'
import { useSeatNamer } from './names'
import { playerOf } from './selectors'
import { useContentBox } from './useZoneFit'

/** Room kept round the row, so the focus ring of the first and last card is not cut by the row's scrolling. */
const PAD_PX = 4

/**
 * The hand dock, the bottom edge of the table: the viewer's cards in one line as tall as the dock allows, side by side
 * while they fit and overlapping just enough when they do not. The cards are the same buttons as everywhere on the
 * table, so a click plays or casts them as the active prompt says.
 */
export const HandDock = memo(function HandDock({ seat }: { seat: PlayerId }) {
  const owner = useSeatNamer()(seat)
  const cards = usePlay((state) => playerOf(state, seat)?.hand.cards ?? null)
  const count = usePlay((state) => playerOf(state, seat)?.hand.count ?? 0)
  const zone = useRef<HTMLDivElement>(null)
  const room = useContentBox(zone)
  const placed = useMemo(() => {
    if (room === null || cards === null) return undefined
    const width = handCardWidth(room.height - 2 * PAD_PX)
    const box: CardBox = { width, height: Math.round(width * CARD_ASPECT) }
    return { box, gap: handFan(room.width - 2 * PAD_PX, cards.length, width).gap }
  }, [room, cards])
  return (
    <section
      aria-label="Your hand"
      data-region="hand"
      className="flex min-h-0 min-w-0 items-center justify-center border-t bg-table p-2"
    >
      <div ref={zone} className="flex size-full min-h-0 min-w-0 items-center justify-center">
        {cards === null ? (
          <HandBacks count={count} />
        ) : cards.length === 0 ? (
          <p className="text-sm text-muted-foreground">Your hand is empty.</p>
        ) : (
          <CardRow
            ids={cards}
            label={`${owner}'s hand`}
            size={placed !== undefined && placed.box.width < 76 ? 'compact' : 'normal'}
            box={placed?.box}
            gap={placed?.gap}
            single
            className="max-w-full px-1 py-1"
          />
        )}
      </div>
    </section>
  )
})

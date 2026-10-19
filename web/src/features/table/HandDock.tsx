import { memo, useMemo, useRef } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import { HandBacks } from './Hand'
import { handLayout } from './handFit'
import { HandFan } from './HandFan'
import { useSeatNamer } from './names'
import { playerOf } from './selectors'
import { useCoarsePointer } from './useCoarsePointer'
import { useContentBox } from './useZoneFit'

/** Room kept round the line, so the focus ring of the first and last card is not cut by the line's scrolling. */
const PAD_PX = 4

/**
 * The hand dock, the bottom edge of the table: the viewer's cards as a fan as tall as the dock allows, side by side
 * while they fit and overlapping just enough when they do not (at least 62 % of a card shows), a hovered or focused card
 * rising by 40 % of its height and opening the preview. The cards are the same buttons as everywhere on the table, so a
 * click plays or casts them as the active prompt says. The fan stays inside the dock's width, so it never reaches the
 * pass controls beside it.
 */
export const HandDock = memo(function HandDock({ seat }: { seat: PlayerId }) {
  const owner = useSeatNamer()(seat)
  const cards = usePlay((state) => playerOf(state, seat)?.hand.cards ?? null)
  const count = usePlay((state) => playerOf(state, seat)?.hand.count ?? 0)
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
      className="flex min-h-0 min-w-0 items-center justify-center border-t bg-table p-2"
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
            size={layout !== undefined && layout.box.width < 76 ? 'compact' : 'normal'}
            layout={layout}
          />
        )}
      </div>
    </section>
  )
})

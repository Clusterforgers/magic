import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { useSeatNamer } from './names'

interface TuckedCardProps {
  id: ObjectId
  size: CardSize
  /** Whether the roving tab stop of the row is this card. */
  tabbable: boolean
  /** Where the card lies behind its host (`top` and `zIndex`). */
  style: CSSProperties
}

/** The seat that controls a tucked card when it is not the seat that controls its host, else undefined. */
function useForeignSeat(id: ObjectId): PlayerId | undefined {
  return usePlay(
    useShallow((state) => {
      const object = state.objectsById[id]
      const host = object?.attached_to === undefined ? undefined : state.objectsById[object.attached_to]
      return object !== undefined && host !== undefined && object.controller !== host.controller
        ? object.controller
        : undefined
    }),
  )
}

/**
 * One attachment behind its host. When another seat controls it (Pacifism on your creature, your Aura on an
 * opponent's) it carries a dashed edge and, above the compact size, its controller's name on the name line; the
 * spoken name has the controller at every size.
 */
export function TuckedCard({ id, size, tabbable, style }: TuckedCardProps) {
  const foreign = useForeignSeat(id)
  const name = useSeatNamer()
  return (
    <div
      data-tucked={id}
      data-foreign={foreign}
      // The ground under each card keeps a dimmed or translucent face from showing the one behind it.
      className="absolute left-0 rounded-md bg-lane"
      style={style}
    >
      <Card id={id} size={size} fit tabbable={tabbable} />
      {foreign === undefined ? null : (
        <>
          {/* Drawn outside the face, whose own edge would fill the gaps of a dash and make the line look solid. */}
          <span
            aria-hidden="true"
            data-slot="tuck-foreign"
            className="pointer-events-none absolute -inset-0.5 rounded-lg border-2 border-dashed border-border-strong"
          />
          {size === 'compact' ? null : (
            <span
              aria-hidden="true"
              data-slot="tuck-owner"
              className="pointer-events-none absolute top-0 right-1 max-w-[45%] truncate rounded-sm bg-foreground px-1 text-[0.6875rem] leading-3 font-semibold text-background"
            >
              {name(foreign)}&rsquo;s
            </span>
          )}
          <span className="sr-only">Controlled by {name(foreign)}</span>
        </>
      )}
    </div>
  )
}

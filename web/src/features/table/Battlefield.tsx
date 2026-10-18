import { memo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import type { RowKind } from './cardModel'
import type { CardSize } from './cardSize'
import { CardRow } from './CardRow'
import { useGhosts } from './fx/hooks'
import { useSeatNamer } from './names'
import { battlefieldRow, isBattlefieldEmpty } from './selectors'

const ROW_NAME: Record<RowKind, string> = {
  lands: 'lands',
  creatures: 'creatures',
  other: 'other permanents',
}

// Creatures sit nearest the middle of the table: below an opponent's lands, above the viewer's.
const ORDER: Record<'near' | 'far', readonly RowKind[]> = {
  near: ['creatures', 'other', 'lands'],
  far: ['lands', 'other', 'creatures'],
}

interface PermanentRowProps {
  seat: PlayerId
  row: RowKind
  label: string
  size: CardSize
}

/** One row of a seat's permanents; it redraws only when the ids in it change. */
const PermanentRow = memo(function PermanentRow({ seat, row, label, size }: PermanentRowProps) {
  const ids = usePlay(useShallow((state) => battlefieldRow(state, seat, row)))
  const ghosts = useGhosts(seat, row)
  return <CardRow ids={ids} ghosts={ghosts} label={label} size={size} />
})

interface BattlefieldProps {
  seat: PlayerId
  /** The size of creatures and other permanents; lands are always compact, since a land is a name and a tap. */
  size: CardSize
  /** `near` is the viewer's own side (creatures on top), `far` an opponent's (lands on top). */
  facing: 'near' | 'far'
}

/** The permanents one seat controls, in rows of lands, creatures and everything else. */
export const Battlefield = memo(function Battlefield({ seat, size, facing }: BattlefieldProps) {
  const owner = useSeatNamer()(seat)
  const empty = usePlay((state) => isBattlefieldEmpty(state, seat))
  return (
    <section
      aria-label={`${owner}'s battlefield`}
      data-battlefield-seat={seat}
      className="grid content-start gap-1"
    >
      {empty ? (
        <p className="px-1 py-3 text-sm text-muted-foreground">No permanents on the battlefield.</p>
      ) : null}
      {ORDER[facing].map((row) => (
        <PermanentRow
          key={row}
          seat={seat}
          row={row}
          label={`${owner}'s ${ROW_NAME[row]}`}
          size={row === 'lands' ? 'compact' : size}
        />
      ))}
    </section>
  )
})

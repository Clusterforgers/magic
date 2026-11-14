/**
 * An opponent's lands as one chip of its pod (`LandFoldContext`): the count and how many are tapped, the land names on
 * hover, and a press that lays the lands out on the board or folds them again. Folded, the board gives the lands'
 * room to its creatures and other permanents.
 */
import { MountainIcon } from 'lucide-react'
import { useContext } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { LandFoldContext, landIdsOf, landNames, landTally, landWords } from './landFold'
import { DENSITY } from './PodParts'
import type { ChipDensity } from './PodParts'

/** The lands chip of a seat's pod; nothing outside a fold or for a seat without lands. */
export function LandSummary({
  seat,
  owner,
  density,
}: {
  seat: PlayerId
  owner: string
  density: ChipDensity
}) {
  const fold = useContext(LandFoldContext)
  const tally = usePlay(
    useShallow((state) => {
      const { count, tapped, names } = landTally(landIdsOf(state, seat), state.objectsById)
      return { count, tapped, names: landNames(names) }
    }),
  )
  if (fold === null || tally.count === 0) return null
  const words = landWords(tally)
  return (
    <button
      type="button"
      data-slot="land-summary"
      data-open={fold.open || undefined}
      aria-expanded={fold.open}
      aria-label={`${fold.open ? 'Fold' : 'Show'} ${owner}'s lands: ${words}`}
      title={tally.names}
      onClick={(event) => {
        // The pod and the board around it attack the seat on a click while an attack is aimed; this press is the chip's.
        event.stopPropagation()
        fold.toggle()
      }}
      className={cn(
        DENSITY[density].chip,
        'cursor-pointer text-muted-foreground outline-offset-2 outline-primary hover:text-foreground focus-visible:outline-2',
        fold.open && 'ring-1 ring-brass',
      )}
    >
      <MountainIcon aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon)} />
      <span aria-hidden="true" className="tabular-nums">
        {tally.count}
        {tally.tapped === 0 ? null : <span className="font-normal"> · {tally.tapped} tapped</span>}
      </span>
    </button>
  )
}

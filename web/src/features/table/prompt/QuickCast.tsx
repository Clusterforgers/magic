import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { ObjectId } from '@/protocol/generated'
import type { CardMarks, CardNote, CardNotes } from './marks'
import { useCardMarks, useCardNotes } from './marks'
import { quickCastIds, quickCastNote } from './quickCast'

/**
 * While no decision of this seat is on screen, keeps its instants and flash cards in hand clickable: a click queues the
 * card for the seat's next priority window (`Session.castWhenAsked`), a second click cancels. The queued card is
 * named on the decision line with a Cancel button. Nothing of this reaches the other seats.
 */
export function QuickCast() {
  const { session } = useGame()
  const ids = usePlay(useShallow(quickCastIds))
  const queued = usePlay((state) => state.castWhenAsked)
  const name = usePlay((state) => (queued === null ? undefined : state.objectsById[queued]?.name))
  const online = usePlay((state) => state.status === 'open')

  const marks = useMemo<CardMarks>(
    () => new Map(ids.map((id) => [id, id === queued ? 'selected' : 'legal'] as const)),
    [ids, queued],
  )
  const notes = useMemo<CardNotes>(
    () => new Map(ids.map((id): [ObjectId, CardNote] => [id, quickCastNote(id === queued)])),
    [ids, queued],
  )
  const pick = (id: ObjectId): void => {
    if (online) session.castWhenAsked(id === queued ? null : id)
  }
  useCardMarks(marks, pick)
  useCardNotes(notes)

  if (queued === null || !ids.includes(queued)) return null
  return (
    <div
      data-slot="quick-cast"
      className="flex max-w-full min-w-0 items-center gap-2 rounded-md bg-muted px-3 py-1.5 sm:max-w-3xl"
    >
      <p aria-live="polite" className="text-sm font-medium">
        Casting {name ?? 'the card'} when you next get priority
      </p>
      <Button size="sm" variant="outline" onClick={() => session.castWhenAsked(null)}>
        Cancel
      </Button>
    </div>
  )
}

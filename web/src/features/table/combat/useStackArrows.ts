import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { usePreviewTarget } from '../preview'
import { stackTopFirst } from '../selectors'
import { groupStack } from '../stackGroups'
import { useStackEntries } from '../useStackEntries'
import type { LineSpec } from './plan'
import { planStackArrows } from './stackArrows'

const NO_LINES: readonly LineSpec[] = []

/** The arrows of the stack item on show (its whole run when folded): the one hovered or focused, else the top. The same array until a line changes. */
export function useStackArrows(): readonly LineSpec[] {
  const entries = useStackEntries()
  const shown = usePreviewTarget()?.id ?? null
  const viewer = usePlay((state) => state.seat)
  const ids = usePlay(useShallow(stackTopFirst))
  const objectsById = usePlay((state) => state.objectsById)
  return useMemo(() => {
    const lines = planStackArrows(entries, shown, viewer, groupStack(ids, objectsById))
    return lines.length === 0 ? NO_LINES : lines
  }, [entries, shown, viewer, ids, objectsById])
}

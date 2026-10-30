import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { usePreviewTarget } from '../preview'
import { useStackEntries } from '../useStackEntries'
import type { LineSpec } from './plan'
import { planStackArrows } from './stackArrows'

const NO_LINES: readonly LineSpec[] = []

/** The arrows of the stack item on show: the one hovered or focused, else the top. The same array until a line changes. */
export function useStackArrows(): readonly LineSpec[] {
  const entries = useStackEntries()
  const shown = usePreviewTarget()?.id ?? null
  const viewer = usePlay((state) => state.seat)
  return useMemo(() => {
    const lines = planStackArrows(entries, shown, viewer)
    return lines.length === 0 ? NO_LINES : lines
  }, [entries, shown, viewer])
}

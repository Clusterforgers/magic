/** The short arrows from a resolving object to what it affects, as lines of the combat overlay's layer. */
import { useMemo } from 'react'
import { useStore } from 'zustand'
import type { EffectArrow } from '../fx/effectArrows'
import { useFx } from '../fx/hooks'
import type { LineSpec } from './plan'

/** One line for each arrow, between the boxes measured when the batch arrived. */
export function effectLines(arrows: readonly EffectArrow[]): LineSpec[] {
  return arrows.map(
    ({ token, kind, from, to }): LineSpec => ({
      key: `effect:${token}:${kind}`,
      kind: 'effect',
      pending: false,
      from: { kind: 'spot', id: `${token}:from`, box: from },
      to: { kind: 'spot', id: `${token}:to`, box: to },
    }),
  )
}

const NO_LINES: readonly LineSpec[] = []

/** The lines of the arrows playing now; the same array while there are none. */
export function useEffectLines(): readonly LineSpec[] {
  const arrows = useStore(useFx().effectArrows.store)
  return useMemo(() => (arrows.length === 0 ? NO_LINES : effectLines(arrows)), [arrows])
}

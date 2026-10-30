/** The arrows of attachments that moved, as lines of the combat overlay's layer. Pure; the store comes in by a hook. */
import { useMemo } from 'react'
import { useStore } from 'zustand'
import type { HostEnd } from '../fx/derive'
import { useFx } from '../fx/hooks'
import type { AttachArrow } from '../fx/arrows'
import type { Anchor, LineSpec } from './plan'

const anchorOf = (end: HostEnd): Anchor =>
  'player' in end
    ? { kind: 'seat', seat: end.player }
    : { kind: 'card', id: end.card, ...(end.seat === undefined ? {} : { seat: end.seat }) }

/**
 * One line for each arrow, from the old host (or, when there was none, the attachment where it lies) to the new one.
 * A new host that is a player ends at that player's pod or tile.
 */
export function attachLines(arrows: readonly AttachArrow[]): LineSpec[] {
  return arrows.flatMap(({ token, change }): LineSpec[] => {
    if (change.to === null) return []
    const from: Anchor =
      change.from === null ? { kind: 'card', id: change.object, seat: change.seat } : anchorOf(change.from)
    return [{ key: `attach:${token}`, kind: 'attach', pending: false, from, to: anchorOf(change.to) }]
  })
}

const NO_LINES: readonly LineSpec[] = []

/** The lines of the arrows playing now; the same array while there are none. */
export function useAttachLines(): readonly LineSpec[] {
  const arrows = useStore(useFx().arrows.store)
  return useMemo(() => (arrows.length === 0 ? NO_LINES : attachLines(arrows)), [arrows])
}

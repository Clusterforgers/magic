import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { ownDecision } from '../ownDecision'
import { askerOf } from './asking'
import type { Asker } from './asking'

/** What asks this seat's decision with the card the view holds for it. */
export interface ShownAsker extends Asker {
  /**
   * The card to draw: the source while the view holds it, else the spell or ability on the stack, drawn as the card
   * the source last was (`identity`) once the source has left its zone.
   */
  object: ObjectView | undefined
  /** The card to light on the board: the source's id. */
  board: ObjectId | undefined
  /** The name on the chip: the server's, else the card's; empty when neither is known. */
  shown: string
}

/** `asker` with the card to draw for it, from its source and its object on the stack as the view holds them. */
export function shownAsker(
  asker: Asker,
  source: ObjectView | undefined,
  stacked: ObjectView | undefined,
): ShownAsker {
  const lastKnown =
    source === undefined &&
    stacked !== undefined &&
    stacked.identity === undefined &&
    asker.identity !== undefined
      ? { ...stacked, identity: asker.identity }
      : stacked
  const object = source ?? lastKnown
  return {
    ...asker,
    object,
    board: source?.id,
    shown: asker.name !== '' ? asker.name : (object?.name ?? ''),
  }
}

/** What asks the decision this seat must answer; null when nothing does. */
export function useAsker(): ShownAsker | null {
  const pending = usePlay(ownDecision)
  const asker = useMemo(() => askerOf(pending), [pending])
  const source = usePlay((state) =>
    asker?.source === undefined ? undefined : state.objectsById[asker.source],
  )
  const stacked = usePlay((state) =>
    asker?.stack === undefined ? undefined : state.objectsById[asker.stack],
  )
  return useMemo(() => (asker === null ? null : shownAsker(asker, source, stacked)), [asker, source, stacked])
}

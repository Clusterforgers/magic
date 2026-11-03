/** The card a choice keeps on show in the table's preview: the one under the pointer or focus, else the last one chosen. */
import { useCallback, useEffect } from 'react'
import type { RefObject } from 'react'
import type { ObjectId } from '@/protocol/generated'
import { anchorOf, usePreviewStore } from '../preview'

/** The card that rests in the preview while nothing is hovered or focused: the last one chosen, or none. */
export function restingCard(chosen: readonly ObjectId[]): ObjectId | null {
  return chosen.at(-1) ?? null
}

/**
 * The element the preview of `id` docks away from: the grid of the choice, which keeps the panel on one side while the
 * pointer crosses its cards and clear of the grid; else the card of the table (`data-object-id`).
 */
function elementOf(id: ObjectId, root: HTMLElement | null): Element | null {
  return root ?? document.querySelector(`[data-object-id="${id}"]`)
}

/** What `useRestingPreview` hands the choice. */
export interface RestingPreview {
  /** Shows the resting card again when nothing else is on show: call it when the pointer or focus leaves a card. */
  restore(): void
}

/**
 * Keeps the last card of `chosen` large at the side while the choice is open, so a card chosen by touch or key stays
 * readable and the card under the pointer replaces it only while it is hovered. The picture is the table's preview
 * (`CardPreview`, `previewFit`); it takes no pointer, Escape dismisses it, and it goes with the choice. A card that is
 * not on the screen (a long list scrolled away, a filter) has no box to dock from and shows nothing.
 */
export function useRestingPreview(
  chosen: readonly ObjectId[],
  root?: RefObject<HTMLElement | null>,
): RestingPreview {
  const preview = usePreviewStore()
  const rest = restingCard(chosen)
  useEffect(() => {
    if (rest === null) return undefined
    const element = elementOf(rest, root?.current ?? null)
    if (element !== null) preview.show(rest, anchorOf(element))
    return () => preview.hide(rest)
  }, [preview, rest, root])
  const restore = useCallback((): void => {
    if (rest === null || preview.get() !== null) return
    const element = elementOf(rest, root?.current ?? null)
    if (element !== null) preview.show(rest, anchorOf(element))
  }, [preview, rest, root])
  return { restore }
}

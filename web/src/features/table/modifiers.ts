/**
 * What changes a card now, as the preview lists it under the rules text (`ObjectView.modifiers`), and the cards a line
 * points at on the board while it is hovered, focused or tapped.
 */
import { create } from 'zustand'
import type { ModifierView, ObjectId, ObjectView } from '@/protocol/generated'

/** One modifier in the preview's words. */
export interface ModifierLine {
  /** Tells the lines of one card apart. */
  key: string
  /** What it does: "+2/+2", "Flying", "Can't block". */
  effect: string
  /** Where it comes from, with the count of identical sources: "Glorious Anthem ×2", "+1/+1 counter ×3". */
  source: string
  /** How long it lasts, or null for as long as its source. */
  duration: string | null
  /** The cards on the table it comes from. */
  sources: readonly ObjectId[]
  /** The whole line for a screen reader and a tooltip. */
  words: string
}

/** The modifiers of `object` in words, in the server's order (pure); empty while nothing changes it. */
export function modifierLines(object: ObjectView): ModifierLine[] {
  return (object.modifiers ?? []).map((modifier: ModifierView, index) => {
    const source = modifier.count > 1 ? `${modifier.source} ×${modifier.count}` : modifier.source
    const duration = modifier.duration ?? null
    const words = `${modifier.effect} from ${source}${duration === null ? '' : `, ${duration}`}`
    return {
      key: `${index}-${words}`,
      effect: modifier.effect,
      source,
      duration,
      sources: modifier.sources ?? [],
      words,
    }
  })
}

interface SourceLit {
  /** The cards a modifier line points at now. */
  ids: readonly ObjectId[]
  /** Points at these cards; none clears. */
  light: (ids: readonly ObjectId[]) => void
}

const NONE: readonly ObjectId[] = []

/** The cards the preview's modifier line under the pointer, focus or a tap points at. */
export const useSourceLit = create<SourceLit>()((set) => ({
  ids: NONE,
  light: (ids) => set({ ids: ids.length === 0 ? NONE : ids }),
}))

/** Whether a modifier line points at the card `id` now. */
export function useIsSourceLit(id: ObjectId): boolean {
  return useSourceLit((state) => state.ids.includes(id))
}

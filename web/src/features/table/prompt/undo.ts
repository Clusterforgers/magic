/** Backing out of a spell or ability being announced: the words of its button and its Escape key. */
import { useEffect, useRef } from 'react'
import type { Zone } from '@/protocol/generated'
import { isTyping } from './usePromptKeys'

/** "Undo the ability" for an ability of a permanent, "Undo the spell" for anything else, by the zone of what asks. */
export function undoWords(zone: Zone | undefined): string {
  return zone === 'Battlefield' ? 'Undo the ability' : 'Undo the spell'
}

/** Whether Escape pressed with `target` focused belongs to something else: a field, menu or dialog outside the prompt. */
export function escapeTaken(target: EventTarget | null): boolean {
  return isTyping(target) && !(target instanceof Element && target.closest('[data-slot="prompt-actions"]') !== null)
}

/**
 * Escape calls `onUndo` while `enabled`, unless a field, menu or dialog outside the prompt has the keyboard (a message
 * to the table) or another control already took the press (an open stack of cards). The prompt's own fields, such as
 * the number of X, do not keep Escape.
 */
export function useUndoKey(onUndo: () => void, enabled = true): void {
  const latest = useRef(onUndo)
  useEffect(() => {
    latest.current = onUndo
  })
  useEffect(() => {
    if (!enabled) return
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape' || event.repeat || event.defaultPrevented || escapeTaken(event.target)) return
      event.preventDefault()
      latest.current()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [enabled])
}

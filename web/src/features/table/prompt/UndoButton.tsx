import type { Ref } from 'react'
import type { Zone } from '@/protocol/generated'
import { Button } from '@/components/ui/button'
import { KeyHint } from './KeyHint'
import { undoWords, useUndoKey } from './undo'

/** Props of `UndoButton`. */
export interface UndoButtonProps {
  /** The zone of the spell or of the source of the ability being announced. */
  zone: Zone | undefined
  onUndo(): void
  ref?: Ref<HTMLButtonElement>
  /** The size classes of the prompt's other buttons. */
  className?: string
}

/**
 * Backs out of the spell or ability being announced, as Arena's Undo does: nothing is paid or moved before the last
 * step, so the card stays where it was and no mana is spent. Escape presses it.
 */
export function UndoButton({ zone, onUndo, ref, className = 'h-10 px-4' }: UndoButtonProps) {
  useUndoKey(onUndo)
  return (
    <Button
      ref={ref}
      type="button"
      variant="outline"
      className={className}
      data-slot="undo"
      aria-keyshortcuts="Escape"
      onClick={onUndo}
    >
      {undoWords(zone)}
      <KeyHint>Esc</KeyHint>
    </Button>
  )
}

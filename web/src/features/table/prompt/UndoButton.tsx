import type { Ref } from 'react'
import type { Zone } from '@/protocol/generated'
import { Button } from '@/components/ui/button'
import { KeyHint } from './KeyHint'
import { undoWords, useUndoKey } from './undo'

/** Props of `UndoButton`. */
export interface UndoButtonProps {
  /** The zone of the spell or of the source of the ability being announced. */
  zone: Zone | undefined
  /** Whether an ability is announced (cycling from a hand) or a spell, whatever the zone; left out, the zone tells. */
  ability?: boolean | undefined
  onUndo(): void
  ref?: Ref<HTMLButtonElement>
  /** The size classes of the prompt's other buttons. */
  className?: string
}

/**
 * Backs out of the spell or ability being announced, as Arena's Undo does, under the word Cancel: nothing is paid or moved before the last
 * step, so the card stays where it was and no mana is spent. Escape presses it.
 */
export function UndoButton({ zone, ability, onUndo, ref, className = 'h-10 px-4' }: UndoButtonProps) {
  useUndoKey(onUndo)
  const words = ability === undefined ? undoWords(zone) : ability ? 'Cancel the ability' : 'Cancel the spell'
  const [verb = words, ...rest] = words.split(' ')
  return (
    <Button
      ref={ref}
      type="button"
      variant="outline"
      className={className}
      data-slot="undo"
      aria-label={words}
      aria-keyshortcuts="Escape"
      onClick={onUndo}
    >
      {/* A narrow band keeps the verb alone (`promptBand.css`), so the answers stay on one row. */}
      {verb}
      <span data-slot="undo-rest">&nbsp;{rest.join(' ')}</span>
      <KeyHint>Esc</KeyHint>
    </Button>
  )
}

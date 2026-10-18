import { useEffect } from 'react'
import { useGame } from '@/features/game/gameContext'
import { isFocusFree } from '../prompt/usePrimaryKey'
import { modeAsked } from './fullControlWords'
import { ModifierChords } from './modifierChords'
import type { Chord } from './modifierChords'

/**
 * Full Control from the keyboard while `enabled`: Control tapped alone asks for the next window (and again takes it
 * back), Control and Shift pressed together ask for it held (and again release it), each when the keys come up.
 * Nothing happens while a field, control, menu or dialog has the keys, the rule Space and Enter follow, nor when
 * the keys are used for anything else (Ctrl+Shift+I, Ctrl-click).
 */
export function useFullControlKeys(enabled: boolean): void {
  const { session } = useGame()
  useEffect(() => {
    if (!enabled) return
    const chords = new ModifierChords()
    const ask = (chord: Chord | null): void => {
      if (chord === null) return
      const mode = modeAsked(chord, session.store.getState().fullControl)
      if (mode !== null) session.setFullControl(mode)
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      chords.keyDown(event)
      if (event.defaultPrevented || !isFocusFree(event.target)) chords.spoil()
    }
    const onKeyUp = (event: KeyboardEvent): void => {
      const chord = chords.keyUp(event)
      if (isFocusFree(event.target)) ask(chord)
    }
    const spoil = (): void => chords.spoil()
    const forget = (): void => chords.reset()
    document.addEventListener('keydown', onKeyDown)
    document.addEventListener('keyup', onKeyUp)
    // A click or a turn of the wheel with Control held is Control used for something else.
    document.addEventListener('pointerdown', spoil, true)
    document.addEventListener('wheel', spoil, true)
    window.addEventListener('blur', forget)
    return () => {
      document.removeEventListener('keydown', onKeyDown)
      document.removeEventListener('keyup', onKeyUp)
      document.removeEventListener('pointerdown', spoil, true)
      document.removeEventListener('wheel', spoil, true)
      window.removeEventListener('blur', forget)
    }
  }, [enabled, session])
}

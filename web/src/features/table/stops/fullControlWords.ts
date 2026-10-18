/** The words of Full Control and what each way of asking for it requests; display and mapping only, the server decides. */
import type { FullControlMode } from '@/protocol/generated'
import type { Chord } from './modifierChords'

/** The state in a word or two, for the toggle and the name a screen reader hears. */
export const FULL_CONTROL_STATE: Record<FullControlMode, string> = {
  Off: 'off',
  NextWindow: 'next window',
  Held: 'held',
}

/** The name of the toggle: "Full Control: off". */
export function fullControlName(mode: FullControlMode): string {
  return `Full Control: ${FULL_CONTROL_STATE[mode]}`
}

/** What each state means and what pressing the toggle does, for the tooltip and a screen reader. */
export const FULL_CONTROL_DESCRIPTION: Record<FullControlMode, string> = {
  Off: 'You are asked only where there is something to do or you set a stop. Press to be asked at every step until you turn it off.',
  NextWindow: 'You are asked at the next window, then Full Control goes off. Press to turn it off.',
  Held: 'You are asked at every step. Press to turn it off.',
}

/** The keys, as the menu and the tooltip print them. */
export const FULL_CONTROL_KEYS = { NextWindow: 'Ctrl', Held: 'Ctrl+Shift' } as const

/** The mode the toggle asks for when pressed: Held from Off, and off again from either mode that is on. */
export function toggledMode(mode: FullControlMode): FullControlMode {
  return mode === 'Off' ? 'Held' : 'Off'
}

/**
 * The mode a key chord asks for, or null for no request. A tap of Control asks for the next window, and again for none;
 * Held is left to Control and Shift, which switch it on and off.
 */
export function modeAsked(chord: Chord, mode: FullControlMode): FullControlMode | null {
  if (chord === 'hold') return mode === 'Held' ? 'Off' : 'Held'
  switch (mode) {
    case 'Off':
      return 'NextWindow'
    case 'NextWindow':
      return 'Off'
    case 'Held':
      return null
  }
}

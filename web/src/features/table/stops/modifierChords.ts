/** Control and Shift on their own: a tap of Control, or the two together. */

/** What the keys meant: Control pressed and let go alone (`tap`), or Control and Shift pressed together and let go (`hold`). */
export type Chord = 'tap' | 'hold'

/** The parts of a key event the tracker reads. */
export interface KeyInfo {
  key: string
  repeat: boolean
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
}

const isModifier = (key: string): boolean => key === 'Control' || key === 'Shift'

/**
 * Tells Control (or Control with Shift) pressed and let go on its own from the same keys used for anything else: a
 * letter (Ctrl+Shift+I), Alt or Meta, a click or a turn of the wheel in between. A chord is told when its first key
 * comes up, so a shortcut that adds a key never counts, and once per gesture.
 */
export class ModifierChords {
  private ctrl = false
  private shift = false
  private both = false
  private spoiled = false
  private fired = false

  /** Feeds a key press. */
  keyDown(event: KeyInfo): void {
    if (event.repeat) return
    // A modifier released out of sight (the window lost focus) is seen from the flags of the next press.
    if (event.key !== 'Control' && !event.ctrlKey) this.ctrl = false
    if (event.key !== 'Shift' && !event.shiftKey) this.shift = false
    if (!isModifier(event.key)) {
      this.spoiled = true
      return
    }
    if (!this.ctrl && !this.shift) this.begin()
    if (event.key === 'Control') this.ctrl = true
    else this.shift = true
    // Alt or Meta held with them (AltGr, a shortcut of the system) is something else.
    if (event.altKey || event.metaKey) this.spoiled = true
    if (this.ctrl && this.shift) this.both = true
  }

  /** Feeds a key release; gives `hold` when Control and Shift were down together and one of them comes up, `tap` for Control alone. */
  keyUp(event: KeyInfo): Chord | null {
    if (!isModifier(event.key)) return null
    let chord: Chord | null = null
    if (!this.spoiled && !this.fired) {
      if (this.both) chord = 'hold'
      else if (event.key === 'Control' && this.ctrl) chord = 'tap'
    }
    if (chord !== null) this.fired = true
    if (event.key === 'Control') this.ctrl = false
    else this.shift = false
    if (!this.ctrl && !this.shift) this.begin()
    return chord
  }

  /** Marks the gesture under way as something else: a pointer press, a wheel turn, a key the page leaves alone. */
  spoil(): void {
    this.spoiled = true
  }

  /** Forgets everything, such as when the window loses focus with a key down. */
  reset(): void {
    this.ctrl = false
    this.shift = false
    this.begin()
  }

  private begin(): void {
    this.both = false
    this.spoiled = false
    this.fired = false
  }
}

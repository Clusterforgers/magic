/** Synthesized sound cues: a few soft sine notes through WebAudio, so no sound file ships. */
import type { SoundCue } from './cues'
import { DEFAULT_VOLUME } from './soundPrefs'

/** One note: its pitch in Hz and when it starts, in seconds after the cue. */
interface Note {
  hz: number
  at: number
}

/** The notes of each cue: a rising pair for the turn, one soft note for a decision, a rising triad for a win. */
export const CUE_NOTES: Record<SoundCue, readonly Note[]> = {
  turn: [
    { hz: 523.25, at: 0 },
    { hz: 783.99, at: 0.09 },
  ],
  decision: [{ hz: 659.25, at: 0 }],
  won: [
    { hz: 523.25, at: 0 },
    { hz: 659.25, at: 0.1 },
    { hz: 783.99, at: 0.2 },
  ],
}

/** How long each note sounds, in seconds. */
export const NOTE_SECONDS = 0.16

/** The loudest a note gets at the default volume (full scale is 1): quiet beside any other sound of the computer. */
export const PEAK_GAIN = 0.05

let context: AudioContext | null = null

/** The page's one audio context, made at the first sound; null where the browser has none or refuses one. */
export function audioContext(): AudioContext | null {
  if (context !== null) return context
  if (typeof window === 'undefined' || typeof window.AudioContext !== 'function') return null
  try {
    context = new window.AudioContext()
  } catch {
    return null
  }
  return context
}

/**
 * Plays `cue` now at `volume` (0 to 1; `DEFAULT_VOLUME` sounds at `PEAK_GAIN`); does nothing where audio is missing,
 * not allowed yet (no click on the page so far) or the volume is 0.
 */
export function playCue(cue: SoundCue, volume = DEFAULT_VOLUME): void {
  if (!(volume > 0)) return
  const ctx = audioContext()
  if (ctx === null) return
  const peak = (PEAK_GAIN * Math.min(1, volume)) / DEFAULT_VOLUME
  try {
    if (ctx.state === 'suspended') void ctx.resume().catch(() => undefined)
    const start = ctx.currentTime
    for (const note of CUE_NOTES[cue]) {
      const oscillator = ctx.createOscillator()
      const gain = ctx.createGain()
      const at = start + note.at
      oscillator.type = 'sine'
      oscillator.frequency.setValueAtTime(note.hz, at)
      gain.gain.setValueAtTime(0, at)
      gain.gain.linearRampToValueAtTime(peak, at + 0.015)
      gain.gain.exponentialRampToValueAtTime(0.0001, at + NOTE_SECONDS)
      oscillator.connect(gain).connect(ctx.destination)
      oscillator.start(at)
      oscillator.stop(at + NOTE_SECONDS + 0.02)
    }
  } catch {
    // A browser that refuses a node plays nothing; the cue is only a courtesy.
  }
}

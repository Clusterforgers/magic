/** Synthesized sound cues: a few soft sine notes through WebAudio, so no sound file ships. */
import type { SoundCue } from './cues'

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

/** The loudest a note gets (full scale is 1): quiet beside any other sound of the computer. */
export const PEAK_GAIN = 0.05

let context: AudioContext | null = null

/** The page's one audio context, made at the first cue; null where the browser has none or refuses one. */
function audio(): AudioContext | null {
  if (context !== null) return context
  if (typeof window === 'undefined' || typeof window.AudioContext !== 'function') return null
  try {
    context = new window.AudioContext()
  } catch {
    return null
  }
  return context
}

/** Plays `cue` now; does nothing where audio is missing or not allowed yet (no click on the page so far). */
export function playCue(cue: SoundCue): void {
  const ctx = audio()
  if (ctx === null) return
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
      gain.gain.linearRampToValueAtTime(PEAK_GAIN, at + 0.015)
      gain.gain.exponentialRampToValueAtTime(0.0001, at + NOTE_SECONDS)
      oscillator.connect(gain).connect(ctx.destination)
      oscillator.start(at)
      oscillator.stop(at + NOTE_SECONDS + 0.02)
    }
  } catch {
    // A browser that refuses a node plays nothing; the cue is only a courtesy.
  }
}

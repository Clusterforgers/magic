// Goal: every cue is a few short sine notes that never get louder than the quiet peak, and a browser without WebAudio, or one that refuses a node, plays nothing and throws nothing.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { CUE_NOTES, NOTE_SECONDS, PEAK_GAIN } from './synth'
import type { SoundCue } from './cues'

const CUES: SoundCue[] = ['turn', 'decision', 'won']

/** A stand-in AudioContext that records the oscillators started and every gain set. */
function fakeAudio(refuse = false) {
  const started: number[] = []
  const gains: number[] = []
  const param = () => ({
    setValueAtTime: (value: number) => gains.push(value),
    linearRampToValueAtTime: (value: number) => gains.push(value),
    exponentialRampToValueAtTime: (value: number) => gains.push(value),
  })
  class FakeContext {
    state = 'running'
    currentTime = 0
    destination = {}
    createOscillator() {
      if (refuse) throw new Error('refused')
      return {
        type: 'sine',
        frequency: { setValueAtTime: () => undefined },
        connect: (gain: unknown) => gain,
        start: (at: number) => started.push(at),
        stop: () => undefined,
      }
    }
    createGain() {
      return { gain: param(), connect: (to: unknown) => to }
    }
    resume() {
      return Promise.resolve()
    }
  }
  return { FakeContext, started, gains }
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.resetModules()
})

describe('the cue notes', () => {
  it.each(CUES)('%s is short and starts within a third of a second', (cue) => {
    const notes = CUE_NOTES[cue]
    expect(notes.length).toBeGreaterThan(0)
    expect(notes.length).toBeLessThanOrEqual(3)
    for (const note of notes) expect(note.at + NOTE_SECONDS).toBeLessThanOrEqual(0.4)
  })
})

describe('playCue', () => {
  it.each(CUES)('plays %s as one oscillator a note, never above the peak gain', async (cue) => {
    const audio = fakeAudio()
    vi.stubGlobal('AudioContext', audio.FakeContext)
    const { playCue } = await import('./synth')
    playCue(cue)
    expect(audio.started).toHaveLength(CUE_NOTES[cue].length)
    expect(Math.max(...audio.gains)).toBeLessThanOrEqual(PEAK_GAIN)
  })

  it('does nothing without WebAudio', async () => {
    vi.stubGlobal('AudioContext', undefined)
    const { playCue } = await import('./synth')
    expect(() => playCue('won')).not.toThrow()
  })

  it('does nothing when the browser refuses a node', async () => {
    vi.stubGlobal('AudioContext', fakeAudio(true).FakeContext)
    const { playCue } = await import('./synth')
    expect(() => playCue('turn')).not.toThrow()
  })
})

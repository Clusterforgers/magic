/**
 * The table's sounds, synthesized with WebAudio so no sound file ships: short tones and filtered noise, each under
 * 400 ms and quiet. `TABLE_VOICES` describes each sound as data (tested without audio); `playTableSound` plays one.
 */
import { audioContext } from './synth'
import type { TableSound } from './tableSounds'

/** A tone: a wave gliding from `hz` to `to` over its length, `at` seconds after the sound starts. */
export interface Tone {
  kind: 'tone'
  wave: OscillatorType
  hz: number
  to?: number
  at: number
  seconds: number
  /** Its loudest point, a share of the sound's peak. */
  level: number
}

/** A breath of noise through a band-pass filter at `hz`: a card sliding, a riffle, the body of a hit. */
export interface Noise {
  kind: 'noise'
  hz: number
  q: number
  at: number
  seconds: number
  level: number
}

/** One part of a sound. */
export type Voice = Tone | Noise

const tone = (wave: OscillatorType, hz: number, at: number, seconds: number, level = 1, to?: number): Tone =>
  to === undefined
    ? { kind: 'tone', wave, hz, at, seconds, level }
    : { kind: 'tone', wave, hz, at, seconds, level, to }
const noise = (hz: number, q: number, at: number, seconds: number, level = 1): Noise => ({
  kind: 'noise',
  hz,
  q,
  at,
  seconds,
  level,
})

/** The parts of every sound. */
export const TABLE_VOICES: Record<TableSound, readonly Voice[]> = {
  // A low drum and the rush of a charge.
  attack: [tone('sine', 110, 0, 0.28, 1, 55), noise(900, 0.8, 0, 0.18, 0.6)],
  // A shield taking it: two bright partials, quickly damped.
  block: [
    tone('triangle', 880, 0, 0.18, 0.7),
    tone('triangle', 1320, 0.005, 0.14, 0.45),
    noise(2400, 2, 0, 0.06, 0.4),
  ],
  // A thump, its pitch falling.
  hit: [tone('sine', 160, 0, 0.22, 1, 60), noise(500, 1, 0, 0.08, 0.5)],
  // A low fall.
  die: [tone('sawtooth', 220, 0, 0.36, 0.35, 70), noise(300, 0.7, 0, 0.25, 0.4)],
  // A rising shimmer.
  cast: [
    tone('sine', 660, 0, 0.18, 0.6),
    tone('sine', 880, 0.06, 0.18, 0.6),
    tone('sine', 1320, 0.12, 0.22, 0.45),
  ],
  // Two notes falling.
  counter: [tone('triangle', 587, 0, 0.14, 0.7), tone('triangle', 392, 0.1, 0.22, 0.7)],
  // A soft settling bell.
  resolve: [tone('triangle', 392, 0, 0.3, 0.6), tone('sine', 784, 0, 0.2, 0.25)],
  // A bright sparkle.
  gain: [tone('sine', 1047, 0, 0.14, 0.45), tone('sine', 1568, 0.07, 0.18, 0.4)],
  // A card set down on the felt.
  land: [noise(700, 1.2, 0, 0.09, 0.8), tone('sine', 90, 0, 0.12, 0.5, 70)],
  // A riffle: four short slides.
  shuffle: [0, 0.06, 0.12, 0.18].map((at) => noise(3000, 1.5, at, 0.05, 0.7)),
  // A card slid off the library.
  draw: [noise(2200, 1, 0, 0.12, 0.7)],
}

/** The loudest a table sound gets at full volume (full scale is 1): beside the cues, never over them. */
export const TABLE_PEAK = 0.06

let noiseBuffer: AudioBuffer | null = null

function noiseOf(ctx: AudioContext): AudioBuffer {
  if (noiseBuffer !== null && noiseBuffer.sampleRate === ctx.sampleRate) return noiseBuffer
  const buffer = ctx.createBuffer(1, Math.ceil(ctx.sampleRate * 0.5), ctx.sampleRate)
  const samples = buffer.getChannelData(0)
  for (let at = 0; at < samples.length; at += 1) samples[at] = Math.random() * 2 - 1
  noiseBuffer = buffer
  return buffer
}

function envelope(ctx: AudioContext, at: number, seconds: number, peak: number): GainNode {
  const gain = ctx.createGain()
  gain.gain.setValueAtTime(0, at)
  gain.gain.linearRampToValueAtTime(peak, at + Math.min(0.012, seconds / 4))
  gain.gain.exponentialRampToValueAtTime(0.0001, at + seconds)
  gain.connect(ctx.destination)
  return gain
}

/**
 * Plays `sound` at `volume` (0 to 1), `delay` seconds from now on the audio clock (no timer waits for it); nothing where
 * audio is missing, not yet allowed or the volume is 0.
 */
export function playTableSound(sound: TableSound, volume: number, delay = 0): void {
  if (!(volume > 0)) return
  const ctx = audioContext()
  if (ctx === null) return
  try {
    if (ctx.state === 'suspended') void ctx.resume().catch(() => undefined)
    const start = ctx.currentTime + Math.max(0, delay)
    for (const voice of TABLE_VOICES[sound]) {
      const at = start + voice.at
      const gain = envelope(ctx, at, voice.seconds, TABLE_PEAK * volume * voice.level)
      if (voice.kind === 'tone') {
        const oscillator = ctx.createOscillator()
        oscillator.type = voice.wave
        oscillator.frequency.setValueAtTime(voice.hz, at)
        if (voice.to !== undefined)
          oscillator.frequency.exponentialRampToValueAtTime(voice.to, at + voice.seconds)
        oscillator.connect(gain)
        oscillator.start(at)
        oscillator.stop(at + voice.seconds + 0.02)
      } else {
        const source = ctx.createBufferSource()
        source.buffer = noiseOf(ctx)
        const filter = ctx.createBiquadFilter()
        filter.type = 'bandpass'
        filter.frequency.setValueAtTime(voice.hz, at)
        filter.Q.setValueAtTime(voice.q, at)
        source.connect(filter).connect(gain)
        source.start(at)
        source.stop(at + voice.seconds + 0.02)
      }
    }
  } catch {
    // A browser that refuses a node plays nothing; the sound is only a courtesy.
  }
}

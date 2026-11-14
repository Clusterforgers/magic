/** Plays the sound cue and the table's sounds of each change of a play store while sounds are on. */
import { useEffect } from 'react'
import type { PlayStore } from '@/play/store'
import { soundCueBetween } from './cues'
import type { SoundCue } from './cues'
import { soundsOn, soundVolume, tableSoundsOn } from './soundPrefs'
import { playCue } from './synth'
import { tableSoundsBetween } from './tableSounds'
import type { TableSound } from './tableSounds'
import { playTableSound } from './tableSynth'

/** The gap between the sounds of one batch on the audio clock, in seconds: a short phrase, not a chord. */
export const SOUND_GAP_SECONDS = 0.09

/** Where the sounds go: the synthesizer, or a test's spies. */
export interface SoundOutput {
  cue(cue: SoundCue, volume: number): void
  table(sound: TableSound, volume: number, delay: number): void
}

const SYNTH: SoundOutput = { cue: playCue, table: playTableSound }

/** Whether a table plays sound cues when nothing says otherwise: everywhere but under vitest. */
export const SOUNDS_BY_DEFAULT = import.meta.env.MODE !== 'test'

/**
 * Subscribes to `store` while `enabled` and hands each cue and the table's sounds of each batch to `output` (the
 * synthesizer by default) at the kept volume, unless muted; the table's sounds also follow their own switch.
 */
export function useSoundCues(store: PlayStore, enabled: boolean, output: SoundOutput = SYNTH): void {
  useEffect(() => {
    if (!enabled) return undefined
    return store.subscribe((next, prev) => {
      if (!soundsOn()) return
      const volume = soundVolume()
      const cue = soundCueBetween(prev, next)
      if (cue !== null) output.cue(cue, volume)
      if (!tableSoundsOn()) return
      tableSoundsBetween(prev, next).forEach((sound, place) =>
        output.table(sound, volume, place * SOUND_GAP_SECONDS),
      )
    })
  }, [store, enabled, output])
}

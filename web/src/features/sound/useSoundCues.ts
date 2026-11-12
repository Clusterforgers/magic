/** Plays the sound cue of each change of a play store while sounds are on. */
import { useEffect } from 'react'
import type { PlayStore } from '@/play/store'
import { soundCueBetween } from './cues'
import type { SoundCue } from './cues'
import { soundsOn } from './soundPrefs'
import { playCue } from './synth'

/** Whether a table plays sound cues when nothing says otherwise: everywhere but under vitest. */
export const SOUNDS_BY_DEFAULT = import.meta.env.MODE !== 'test'

/** Subscribes to `store` while `enabled` and hands each cue to `play` (the synthesizer by default) unless muted. */
export function useSoundCues(store: PlayStore, enabled: boolean, play: (cue: SoundCue) => void = playCue): void {
  useEffect(() => {
    if (!enabled) return undefined
    return store.subscribe((next, prev) => {
      const cue = soundCueBetween(prev, next)
      if (cue !== null && soundsOn()) play(cue)
    })
  }, [store, enabled, play])
}

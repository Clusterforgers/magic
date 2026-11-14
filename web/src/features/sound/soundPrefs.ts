/** Whether the table plays its sound cues, kept in this browser's `localStorage`; on until muted. */
import { useSyncExternalStore } from 'react'

/** The `localStorage` key of the mute: `off` while muted. */
export const SOUNDS_KEY = 'mtg.sounds'

const listeners = new Set<() => void>()

/** Whether sound cues play, read now. */
export function soundsOn(): boolean {
  try {
    return localStorage.getItem(SOUNDS_KEY) !== 'off'
  } catch {
    return true
  }
}

/** Turns the cues on or off in this browser, for every game and page. */
export function setSoundsOn(on: boolean): void {
  try {
    if (on) localStorage.removeItem(SOUNDS_KEY)
    else localStorage.setItem(SOUNDS_KEY, 'off')
  } catch {
    // Storage that is blocked keeps the default; the switch still reads it back.
  }
  for (const listener of listeners) listener()
}

function subscribe(onChange: () => void): () => void {
  listeners.add(onChange)
  const onStorage = (event: StorageEvent) => {
    if (event.key === SOUNDS_KEY) onChange()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(onChange)
    window.removeEventListener('storage', onStorage)
  }
}

/** Whether sound cues play; follows a change made here or in another tab. */
export function useSoundsOn(): boolean {
  return useSyncExternalStore(subscribe, soundsOn, () => true)
}

/** The `localStorage` key of the volume, a share of full scale from 0 to 1. */
export const VOLUME_KEY = 'mtg.sounds.volume'

/** The volume while none is chosen. */
export const DEFAULT_VOLUME = 0.7

/** The `localStorage` key of the table's sounds (casts, attacks, draws): `off` while only the cues play. */
export const TABLE_SOUNDS_KEY = 'mtg.sounds.table'

const KEYS = new Set([SOUNDS_KEY, VOLUME_KEY, TABLE_SOUNDS_KEY])

function subscribeAll(onChange: () => void): () => void {
  listeners.add(onChange)
  const onStorage = (event: StorageEvent) => {
    if (event.key !== null && KEYS.has(event.key)) onChange()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(onChange)
    window.removeEventListener('storage', onStorage)
  }
}

/** The volume of every sound, 0 to 1; `DEFAULT_VOLUME` while none is kept or the kept one is no number. */
export function soundVolume(): number {
  try {
    const kept = localStorage.getItem(VOLUME_KEY)
    const value = kept === null ? Number.NaN : Number(kept)
    return Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : DEFAULT_VOLUME
  } catch {
    return DEFAULT_VOLUME
  }
}

/** Keeps the volume (clamped to 0 to 1) for every game and page of this browser. */
export function setSoundVolume(volume: number): void {
  try {
    localStorage.setItem(
      VOLUME_KEY,
      String(Math.min(1, Math.max(0, Number.isFinite(volume) ? volume : DEFAULT_VOLUME))),
    )
  } catch {
    // Storage that is blocked keeps the default volume.
  }
  for (const listener of listeners) listener()
}

/** Whether the table's sounds play beside the cues; on until switched off. */
export function tableSoundsOn(): boolean {
  try {
    return localStorage.getItem(TABLE_SOUNDS_KEY) !== 'off'
  } catch {
    return true
  }
}

/** Turns the table's sounds on or off in this browser. */
export function setTableSoundsOn(on: boolean): void {
  try {
    if (on) localStorage.removeItem(TABLE_SOUNDS_KEY)
    else localStorage.setItem(TABLE_SOUNDS_KEY, 'off')
  } catch {
    // Blocked storage keeps them on.
  }
  for (const listener of listeners) listener()
}

/** The volume, followed across tabs. */
export function useSoundVolume(): number {
  return useSyncExternalStore(subscribeAll, soundVolume, () => DEFAULT_VOLUME)
}

/** Whether the table's sounds play, followed across tabs. */
export function useTableSoundsOn(): boolean {
  return useSyncExternalStore(subscribeAll, tableSoundsOn, () => true)
}

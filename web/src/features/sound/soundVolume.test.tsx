// Goal: the volume and the table's sounds are kept per browser with safe defaults, set from the settings page, and a table hands each batch's sounds to the output at the kept volume, staggered, only while sounds and the table's sounds are on.
import { act, fireEvent, render, screen } from '@testing-library/react'
import { useMemo } from 'react'
import { afterEach, describe, expect, it } from 'vitest'
import { appendLog, createPlayStore } from '@/play/store'
import { ME, miniView } from '@/test-utils/miniTable'
import { hid } from '@/test-utils/turnEvents'
import {
  DEFAULT_VOLUME,
  setSoundsOn,
  setSoundVolume,
  setTableSoundsOn,
  soundVolume,
  tableSoundsOn,
  TABLE_SOUNDS_KEY,
  VOLUME_KEY,
} from './soundPrefs'
import { SoundToggle } from './SoundToggle'
import { SOUND_GAP_SECONDS, useSoundCues } from './useSoundCues'
import type { SoundOutput } from './useSoundCues'

afterEach(() => localStorage.clear())

describe('the volume', () => {
  it.each([
    [null, DEFAULT_VOLUME],
    ['0.3', 0.3],
    ['7', 1],
    ['-1', 0],
    ['loud', DEFAULT_VOLUME],
  ])('reads %s as %f', (kept, volume) => {
    if (kept !== null) localStorage.setItem(VOLUME_KEY, kept)
    expect(soundVolume()).toBe(volume)
  })

  it('is set from the settings page', () => {
    render(<SoundToggle />)
    fireEvent.change(screen.getByRole('slider', { name: 'Volume' }), { target: { value: '40' } })
    expect(soundVolume()).toBe(0.4)
  })
})

describe("the table's sounds", () => {
  it('are on until switched off, and the switch waits for the sounds', () => {
    expect(tableSoundsOn()).toBe(true)
    setTableSoundsOn(false)
    expect(localStorage.getItem(TABLE_SOUNDS_KEY)).toBe('off')
    render(<SoundToggle />)
    const table = screen.getByRole('switch', { name: /^Also play the table/ })
    expect(table).not.toBeChecked()
    act(() => setSoundsOn(false))
    expect(table).toHaveAttribute('aria-disabled', 'true')
  })
})

function Listener({ store, output }: { store: ReturnType<typeof createPlayStore>; output: SoundOutput }) {
  const stable = useMemo(() => output, [output])
  useSoundCues(store, true, stable)
  return null
}

describe('useSoundCues with the table', () => {
  const batch = (store: ReturnType<typeof createPlayStore>) => {
    const state = store.getState()
    const seq = state.seq ?? 0
    const events = [hid({ CardDrawn: {} } as never), hid({ LibraryShuffled: {} } as never)]
    store.setState({ view: { ...miniView() }, seq: seq + 2, log: appendLog(state.log, seq, events) })
  }

  it.each([
    [
      true,
      true,
      [
        ['shuffle', 0.3, 0],
        ['draw', 0.3, SOUND_GAP_SECONDS],
      ],
    ],
    [true, false, []],
    [false, true, []],
  ])('with sounds %s and table %s plays %j', (sounds, table, heard) => {
    setSoundsOn(sounds)
    setTableSoundsOn(table)
    setSoundVolume(0.3)
    const store = createPlayStore()
    store.setState({ seat: ME, view: miniView(), seq: 4 })
    const played: unknown[] = []
    const output: SoundOutput = { cue: () => undefined, table: (...args) => played.push(args) }
    render(<Listener store={store} output={output} />)
    act(() => batch(store))
    expect(played).toEqual(heard)
  })
})

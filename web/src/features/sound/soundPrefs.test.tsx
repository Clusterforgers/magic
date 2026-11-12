// Goal: the sound cues are on until muted, the mute is kept in this browser and followed by every switch at once, blocked storage keeps them on without a throw, and a muted table plays no cue while an unmuted one plays each cue of its store.
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createPlayStore } from '@/play/store'
import { decisionFor } from '@/test-utils/decisions'
import { ME, miniView } from '@/test-utils/miniTable'
import { setSoundsOn, soundsOn, SOUNDS_KEY } from './soundPrefs'
import { SoundToggle } from './SoundToggle'
import { useSoundCues } from './useSoundCues'
import type { SoundCue } from './cues'

afterEach(() => {
  localStorage.clear()
  vi.restoreAllMocks()
})

describe('the sound preference', () => {
  it('is on until muted and kept under its key', () => {
    expect(soundsOn()).toBe(true)
    setSoundsOn(false)
    expect(localStorage.getItem(SOUNDS_KEY)).toBe('off')
    expect(soundsOn()).toBe(false)
    setSoundsOn(true)
    expect(localStorage.getItem(SOUNDS_KEY)).toBeNull()
  })

  it('stays on when storage is blocked', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(() => setSoundsOn(false)).not.toThrow()
    expect(soundsOn()).toBe(true)
  })

  it('is switched from the settings toggle', async () => {
    render(<SoundToggle />)
    const toggle = screen.getByRole('switch')
    expect(toggle).toBeChecked()
    await userEvent.click(toggle)
    expect(soundsOn()).toBe(false)
    expect(toggle).not.toBeChecked()
  })
})

function Listener({
  store,
  play,
}: {
  store: ReturnType<typeof createPlayStore>
  play: (cue: SoundCue) => void
}) {
  useSoundCues(store, true, play)
  return null
}

describe('useSoundCues', () => {
  it.each([
    [true, ['decision']],
    [false, []],
  ])('with sounds on %s plays %j for a decision', (on, heard) => {
    setSoundsOn(on)
    const store = createPlayStore()
    store.setState({ seat: ME, view: miniView() })
    const played: SoundCue[] = []
    const play = (cue: SoundCue) => played.push(cue)
    render(<Listener store={store} play={play} />)
    act(() => store.setState({ pending: { id: 1, decision: decisionFor('Priority', ME) } }))
    expect(played).toEqual(heard)
  })
})

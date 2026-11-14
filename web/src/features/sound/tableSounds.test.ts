// Goal: a batch plays the table's sounds of its public events (a draw or shuffle of another seat by its hidden event), each kind once, the strongest first, at most three; a snapshot or a change that is no batch plays none; every voice is short and quiet; and the synthesizer plays nothing without audio or at volume 0.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { appendLog, initialPlayState } from '@/play/store'
import type { PlayState } from '@/play/store'
import type { EventView } from '@/protocol/generated'
import { hid, pub } from '@/test-utils/turnEvents'
import { MAX_TABLE_SOUNDS, SOUND_ORDER, soundOf, soundsOf, tableSoundsBetween } from './tableSounds'
import type { TableSound } from './tableSounds'
import { TABLE_PEAK, TABLE_VOICES } from './tableSynth'

const EVENTS: ReadonlyArray<[string, EventView, TableSound | null]> = [
  ['an attack', pub({ AttackersDeclared: { player: 0, attackers: [5] } } as never), 'attack'],
  ['no attack', pub({ AttackersDeclared: { player: 0, attackers: [] } } as never), null],
  ['a block', pub({ BlockersDeclared: { player: 1, blocks: [[6, 5]] } }), 'block'],
  ['no block', pub({ BlockersDeclared: { player: 1, blocks: [] } }), null],
  ['damage', pub({ DamageDealt: {} } as never), 'hit'],
  ['a death', pub({ CreatureDied: {} } as never), 'die'],
  ['a cast', pub({ SpellCast: {} } as never), 'cast'],
  ['a resolution', pub({ ResolutionEnded: { object: 9, outcome: 'Resolved' } }), 'resolve'],
  ['a counter', pub({ ResolutionEnded: { object: 9, outcome: 'Countered' } }), 'counter'],
  ['a fizzle', pub({ ResolutionEnded: { object: 9, outcome: 'Fizzled' } }), 'counter'],
  ['life gained', pub({ LifeGained: {} } as never), 'gain'],
  ['a land', pub({ LandPlayed: {} } as never), 'land'],
  ['a shuffle', pub({ LibraryShuffled: {} } as never), 'shuffle'],
  ['a hidden shuffle', hid({ LibraryShuffled: {} } as never), 'shuffle'],
  ['a draw', pub({ CardDrawn: {} } as never), 'draw'],
  ['a hidden draw', hid({ CardDrawn: {} } as never), 'draw'],
  ['a pass', pub({ PriorityPassed: { player: 0 } }), null],
]

describe('soundOf', () => {
  it.each(EVENTS)('hears %s as %s', (_, event, sound) => {
    expect(soundOf(event)).toBe(sound)
  })
})

describe('soundsOf', () => {
  it('plays each kind once, the strongest first, at most three', () => {
    const events = EVENTS.map(([, event]) => event)
    expect(soundsOf(events)).toEqual(SOUND_ORDER.slice(0, MAX_TABLE_SOUNDS))
    expect(soundsOf([...events].reverse())).toEqual(SOUND_ORDER.slice(0, MAX_TABLE_SOUNDS))
  })

  it('plays a cast and its resolution as one phrase', () => {
    const cast = must(EVENTS[6])[1]
    const resolved = must(EVENTS[7])[1]
    expect(soundsOf([cast, cast, resolved])).toEqual(['cast', 'resolve'])
  })

  it('plays nothing for a batch of nothing it hears', () => {
    expect(soundsOf([pub({ PriorityPassed: { player: 1 } })])).toEqual([])
  })
})

function must<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('missing')
  return value
}

describe('tableSoundsBetween', () => {
  const view = { players: [] } as unknown as NonNullable<PlayState['view']>
  const base = (): PlayState => ({ ...initialPlayState(), view, seq: 10, seat: 0 })
  const draw = hid({ CardDrawn: {} } as never)

  it('hears a batch, and the same for every seat', () => {
    const prev = base()
    const next = { ...prev, view: { ...view }, seq: 11, log: appendLog(prev.log, 10, [draw]) }
    expect(tableSoundsBetween(prev, next)).toEqual(['draw'])
    expect(tableSoundsBetween({ ...prev, seat: 1 }, { ...next, seat: 1 })).toEqual(['draw'])
  })

  it('hears nothing from a snapshot or a change that is no batch', () => {
    const prev = base()
    const snapshot = {
      ...prev,
      view: { ...view },
      seq: 11,
      snapshots: 1,
      log: appendLog(prev.log, 10, [draw]),
    }
    expect(tableSoundsBetween(prev, snapshot)).toEqual([])
    expect(tableSoundsBetween(prev, { ...prev, chat: [] })).toEqual([])
  })
})

describe('the voices', () => {
  it.each(SOUND_ORDER)('keeps %s short and quiet', (sound) => {
    const voices = TABLE_VOICES[sound]
    expect(voices.length).toBeGreaterThan(0)
    for (const voice of voices) {
      expect(voice.at + voice.seconds).toBeLessThanOrEqual(0.4)
      expect(voice.level).toBeGreaterThan(0)
      expect(voice.level).toBeLessThanOrEqual(1)
    }
    expect(TABLE_PEAK).toBeLessThanOrEqual(0.08)
  })
})

describe('playTableSound', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.resetModules()
  })

  it('does nothing without WebAudio', async () => {
    vi.stubGlobal('AudioContext', undefined)
    const { playTableSound } = await import('./tableSynth')
    expect(() => playTableSound('attack', 1)).not.toThrow()
  })

  it('makes no audio at volume 0', async () => {
    const made = vi.fn()
    vi.stubGlobal(
      'AudioContext',
      class {
        constructor() {
          made()
        }
      },
    )
    const { playTableSound } = await import('./tableSynth')
    playTableSound('cast', 0)
    expect(made).not.toHaveBeenCalled()
  })
})

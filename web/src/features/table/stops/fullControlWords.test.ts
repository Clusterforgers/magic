// Goal: every Full Control mode has its words (the name carries the state, the description says what pressing does), the toggle asks for Held from Off and for Off from anything on, and a key chord asks for the next window, Held or nothing as the table of modes says.
import { describe, expect, it } from 'vitest'
import type { FullControlMode } from '@/protocol/generated'
import {
  FULL_CONTROL_DESCRIPTION,
  FULL_CONTROL_KEYS,
  FULL_CONTROL_STATE,
  fullControlName,
  modeAsked,
  toggledMode,
} from './fullControlWords'
import type { Chord } from './modifierChords'

const MODES: FullControlMode[] = ['Off', 'NextWindow', 'Held']

describe('the words', () => {
  it.each([
    ['Off', 'Full Control: off'],
    ['NextWindow', 'Full Control: next window'],
    ['Held', 'Full Control: held'],
  ] satisfies [FullControlMode, string][])('name %s "%s"', (mode, name) => {
    expect(fullControlName(mode)).toBe(name)
  })

  it.each(MODES)('has a state word and a description for %s', (mode) => {
    expect(FULL_CONTROL_STATE[mode]).not.toBe('')
    expect(FULL_CONTROL_DESCRIPTION[mode]).toMatch(/\.$/)
    expect(FULL_CONTROL_DESCRIPTION[mode]).toMatch(/^[A-Z]/)
  })

  it('has three different state words and descriptions', () => {
    expect(new Set(Object.values(FULL_CONTROL_STATE)).size).toBe(3)
    expect(new Set(Object.values(FULL_CONTROL_DESCRIPTION)).size).toBe(3)
  })

  it('says what pressing does in the description of the mode it is pressed in', () => {
    expect(FULL_CONTROL_DESCRIPTION.Off).toMatch(/Press to be asked at every step/)
    expect(FULL_CONTROL_DESCRIPTION.NextWindow).toMatch(/Press to turn it off/)
    expect(FULL_CONTROL_DESCRIPTION.Held).toMatch(/Press to turn it off/)
  })

  it('names the keys the way the menu prints them', () => {
    expect(FULL_CONTROL_KEYS).toEqual({ NextWindow: 'Ctrl', Held: 'Ctrl+Shift' })
  })
})

describe('what the toggle asks for', () => {
  it.each([
    ['Off', 'Held'],
    ['NextWindow', 'Off'],
    ['Held', 'Off'],
  ] satisfies [FullControlMode, FullControlMode][])('pressed in %s it asks for %s', (mode, asked) => {
    expect(toggledMode(mode)).toBe(asked)
  })
})

describe('what a key chord asks for', () => {
  it.each([
    ['tap', 'Off', 'NextWindow'],
    ['tap', 'NextWindow', 'Off'],
    ['tap', 'Held', null],
    ['hold', 'Off', 'Held'],
    ['hold', 'NextWindow', 'Held'],
    ['hold', 'Held', 'Off'],
  ] satisfies [Chord, FullControlMode, FullControlMode | null][])(
    '%s in %s asks for %s',
    (chord, mode, asked) => {
      expect(modeAsked(chord, mode)).toBe(asked)
    },
  )

  it('never asks for the mode the seat is already in', () => {
    for (const chord of ['tap', 'hold'] satisfies Chord[]) {
      for (const mode of MODES) expect(modeAsked(chord, mode)).not.toBe(mode)
    }
  })
})

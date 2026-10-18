// Goal: Control pressed and let go alone is a tap, Control and Shift pressed together and let go is a hold, each told once when the first key comes up, and the same keys used with a letter, Alt or Meta, a click, a repeat or a lost window are never told.
import { describe, expect, it } from 'vitest'
import { ModifierChords } from './modifierChords'
import type { Chord, KeyInfo } from './modifierChords'

/**
 * Plays a script on a keyboard of the real kind: `down K [alt] [meta]`, `up K`, `repeat K`, `lose K` (the key goes up
 * where the page cannot see it), `spoil` (a click or a wheel turn) and `reset` (the window loses focus). Gives what
 * the tracker told, in order.
 */
function play(script: string[]): Chord[] {
  const chords = new ModifierChords()
  const held = new Set<string>()
  const told: Chord[] = []
  const info = (key: string, extra: string[], repeat: boolean): KeyInfo => ({
    key,
    repeat,
    altKey: extra.includes('alt') || held.has('Alt'),
    ctrlKey: held.has('Control'),
    metaKey: extra.includes('meta') || held.has('Meta'),
    shiftKey: held.has('Shift'),
  })
  for (const line of script) {
    const [action, key = '', ...extra] = line.split(' ')
    switch (action) {
      case 'down':
        held.add(key)
        chords.keyDown(info(key, extra, false))
        break
      case 'repeat':
        chords.keyDown(info(key, extra, true))
        break
      case 'up': {
        held.delete(key)
        const chord = chords.keyUp(info(key, extra, false))
        if (chord !== null) told.push(chord)
        break
      }
      case 'lose':
        held.delete(key)
        break
      case 'spoil':
        chords.spoil()
        break
      case 'reset':
        chords.reset()
        break
      default:
        throw new Error(`unknown step ${line}`)
    }
  }
  return told
}

describe('a tap of Control', () => {
  it.each([
    ['pressed and let go', ['down Control', 'up Control'], ['tap']],
    [
      'held for a while, the key repeating',
      ['down Control', 'repeat Control', 'repeat Control', 'up Control'],
      ['tap'],
    ],
    ['twice', ['down Control', 'up Control', 'down Control', 'up Control'], ['tap', 'tap']],
    [
      'after a gesture that was something else',
      ['down Control', 'down a', 'up a', 'up Control', 'down Control', 'up Control'],
      ['tap'],
    ],
    [
      'after a hold',
      ['down Control', 'down Shift', 'up Shift', 'up Control', 'down Control', 'up Control'],
      ['hold', 'tap'],
    ],
    [
      'after a key went up unseen, which the next press shows',
      ['down Control', 'lose Control', 'down a', 'up a', 'down Control', 'up Control'],
      ['tap'],
    ],
  ] satisfies [string, string[], Chord[]][])('is told when it is %s', (_what, script, told) => {
    expect(play(script)).toEqual(told)
  })

  it.each([
    ['with a letter, let go first', ['down Control', 'down a', 'up a', 'up Control']],
    ['with a letter, Control let go first', ['down Control', 'down a', 'up Control', 'up a']],
    ['with Enter', ['down Control', 'down Enter', 'up Enter', 'up Control']],
    ['with Alt', ['down Control', 'down Alt', 'up Alt', 'up Control']],
    ['with Alt already down', ['down Alt', 'down Control', 'up Control', 'up Alt']],
    ['with the Alt of AltGr in its flags', ['down Control alt', 'up Control']],
    ['with Meta in its flags', ['down Control meta', 'up Control']],
    ['with a click or a wheel turn between', ['down Control', 'spoil', 'up Control']],
    ['with the window losing focus between', ['down Control', 'reset', 'up Control']],
    ['pressed out of sight, so only its release is seen', ['up Control']],
  ])('is not told when Control is used %s', (_what, script) => {
    expect(play(script)).toEqual([])
  })
})

describe('Control and Shift together', () => {
  it.each([
    ['Control first, Shift let go first', ['down Control', 'down Shift', 'up Shift', 'up Control']],
    ['Control first, Control let go first', ['down Control', 'down Shift', 'up Control', 'up Shift']],
    ['Shift first, Control let go first', ['down Shift', 'down Control', 'up Control', 'up Shift']],
    ['Shift first, Shift let go first', ['down Shift', 'down Control', 'up Shift', 'up Control']],
  ])('are told as a hold, once, with %s', (_what, script) => {
    expect(play(script)).toEqual(['hold'])
  })

  it('are told when the first key comes up, not when the second goes down', () => {
    expect(play(['down Control', 'down Shift'])).toEqual([])
    expect(play(['down Control', 'down Shift', 'up Shift'])).toEqual(['hold'])
  })

  it('are told once per gesture, however often Shift is pressed again while Control stays down', () => {
    const script = ['down Control', 'down Shift', 'up Shift', 'down Shift', 'up Shift', 'up Control']
    expect(play(script)).toEqual(['hold'])
  })

  it('are told again for a new gesture', () => {
    const once = ['down Control', 'down Shift', 'up Shift', 'up Control']
    expect(play([...once, ...once])).toEqual(['hold', 'hold'])
  })

  it.each([
    [
      'a letter, as in Ctrl+Shift+I',
      ['down Control', 'down Shift', 'down i', 'up i', 'up Shift', 'up Control'],
    ],
    ['a letter let go last', ['down Control', 'down Shift', 'down i', 'up Shift', 'up Control', 'up i']],
    ['Alt', ['down Control', 'down Shift', 'down Alt', 'up Alt', 'up Shift', 'up Control']],
    ['Meta in the flags', ['down Control meta', 'down Shift', 'up Shift', 'up Control']],
    ['a click', ['down Control', 'down Shift', 'spoil', 'up Shift', 'up Control']],
    ['the window losing focus', ['down Control', 'down Shift', 'reset', 'up Shift', 'up Control']],
  ])('are not told with %s', (_what, script) => {
    expect(play(script)).toEqual([])
  })

  it('leave Shift alone to be Shift', () => {
    expect(play(['down Shift', 'up Shift'])).toEqual([])
    expect(play(['down Shift', 'down a', 'up a', 'up Shift'])).toEqual([])
  })
})

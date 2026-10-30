// Goal: the Roll row offers a coin, a d6, a d20 and a die of 2 to 100 faces, previews "Rolls a d20; everyone sees the result" before anything is sent, refuses a size outside the range or a fraction with the reason, sends exactly the roll it names, and the log sentence reads a result as "rolls a d20: 14" or "flips a coin: heads".
import { describe, expect, it } from 'vitest'
import type { ManualRoll } from '@/protocol/generated'
import { ROLL_PICKS, rollFace, rollKind, rollPreview, rollSentence, SIDES } from './roll'
import type { RollPick } from './roll'

describe('rollKind', () => {
  it.each<[RollPick, number, ManualRoll | null]>([
    ['coin', 10, { Coin: {} }],
    ['d6', 10, { Die: { sides: 6 } }],
    ['d20', 10, { Die: { sides: 20 } }],
    ['custom', 2, { Die: { sides: 2 } }],
    ['custom', 37, { Die: { sides: 37 } }],
    ['custom', 100, { Die: { sides: 100 } }],
    ['custom', 1, null],
    ['custom', 0, null],
    ['custom', 101, null],
    ['custom', -5, null],
    ['custom', 6.5, null],
    ['custom', Number.NaN, null],
  ])('%s with %s faces is %j', (pick, sides, kind) => {
    expect(rollKind(pick, sides)).toEqual(kind)
  })

  it('lists the choices in the order of the row', () => {
    expect(ROLL_PICKS.map((choice) => choice.label)).toEqual(['Coin', 'd6', 'd20', 'd N'])
    expect(SIDES).toMatchObject({ min: 2, max: 100 })
  })
})

describe('rollPreview', () => {
  it.each<[RollPick, number, string, string, ManualRoll]>([
    ['d20', 10, 'Rolls a d20; everyone sees the result', 'Roll a d20', { Die: { sides: 20 } }],
    ['d6', 10, 'Rolls a d6; everyone sees the result', 'Roll a d6', { Die: { sides: 6 } }],
    ['coin', 10, 'Flips a coin; everyone sees the result', 'Flip a coin', { Coin: {} }],
    ['custom', 12, 'Rolls a d12; everyone sees the result', 'Roll a d12', { Die: { sides: 12 } }],
  ])('%s previews "%s" and sends "%s"', (pick, sides, preview, apply, kind) => {
    expect(rollPreview(pick, sides)).toEqual({ preview, apply, action: { Roll: { kind } } })
  })

  it.each([1, 101, 2.5, Number.NaN])('refuses %s faces with the range', (sides) => {
    expect(rollPreview('custom', sides)).toEqual({ problem: 'Choose a die of 2 to 100 faces.' })
  })
})

describe('the sentence', () => {
  it.each<[ManualRoll, number | undefined, string]>([
    [{ Die: { sides: 20 } }, 14, 'Ana rolls a d20: 14'],
    [{ Die: { sides: 100 } }, 1, 'Ana rolls a d100: 1'],
    [{ Coin: {} }, 1, 'Ana flips a coin: heads'],
    [{ Coin: {} }, 2, 'Ana flips a coin: tails'],
    [{ Die: { sides: 6 } }, undefined, 'Ana rolls a d6'],
  ])('%j with %s reads "%s"', (kind, result, text) => {
    expect(rollSentence('Ana', kind, result)).toBe(text)
  })

  it('names a face of a coin and of a die', () => {
    expect(rollFace({ Coin: {} }, 2)).toBe('tails')
    expect(rollFace({ Die: { sides: 2 } }, 2)).toBe('2')
  })
})

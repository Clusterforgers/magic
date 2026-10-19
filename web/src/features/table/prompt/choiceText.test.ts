// Goal: the questions of effects are put in the words of the card when the engine gave them and in plain ones when not, the number keys cover the first ten entries, a mode is chosen, repeated or taken back by the rules of its bounds, and the answer lists the chosen modes in the order they happen.
import { describe, expect, it } from 'vitest'
import {
  askerWords,
  modeBounds,
  modeKeysWords,
  modeLabel,
  modesAnswer,
  modesSentence,
  numberKey,
  numberKeysWords,
  optionLabel,
  optionSentence,
  pressedMode,
  yesNoSentence,
} from './choiceText'

describe('the questions', () => {
  it.each([
    ['Rhystic Study', 'Pay {1}?', 'Rhystic Study: Pay {1}?'],
    ['Rhystic Study', '', 'Rhystic Study: do you want to do this?'],
  ])('%s asking "%s" reads "%s"', (asker, text, sentence) => {
    expect(yesNoSentence(asker, text)).toBe(sentence)
  })

  it.each([
    ['Treasure', 'Choose a color.', 'Treasure: Choose a color.'],
    ['Treasure', '', 'Treasure: choose one.'],
  ])('%s asking "%s" reads "%s"', (asker, text, sentence) => {
    expect(optionSentence(asker, text)).toBe(sentence)
  })

  it('names a source the view does not hold as a card', () => {
    expect(askerWords(undefined)).toBe('A card')
    expect(askerWords('Mulldrifter')).toBe('Mulldrifter')
  })
})

describe('the number keys', () => {
  it.each([
    [0, '1'],
    [8, '9'],
    [9, '0'],
    [10, undefined],
  ])('entry %i has the key %s', (index, key) => {
    expect(numberKey(index)).toBe(key)
  })

  it.each([
    [1, 'Press 1 to choose it.'],
    [2, 'Press 1 or 2 to choose.'],
    [5, 'Press 1 to 5 to choose.'],
    [9, 'Press 1 to 9 to choose.'],
    [10, 'Press 1 to 9, or 0, to choose.'],
    [12, 'Press 1 to 9, or 0, to choose one of the first ten.'],
  ])('a list of %i is told as "%s"', (count, words) => {
    expect(numberKeysWords(count)).toBe(words)
  })

  it.each([
    [3, true, 'Press 1 to 3 to choose.'],
    [3, false, 'Press 1 to 3 to choose a mode or take it back.'],
    [1, false, 'Press 1 to choose a mode or take it back.'],
    [11, false, 'Press 1 to 9, or 0, to choose a mode or take it back.'],
  ])('%i modes, one asked for: %s, are told as "%s"', (count, single, words) => {
    expect(modeKeysWords(count, single)).toBe(words)
  })
})

describe('what an answer is called', () => {
  const seat = (n: number): string => ['Ana', 'Bo', 'Cy'][n] ?? '?'

  it.each([
    [{ Player: 0 }, 0, 'Ana (you)'],
    [{ Player: 1 }, 0, 'Bo'],
    [{ Player: 1 }, null, 'Bo'],
    [{ Color: 'Red' }, 0, 'Red'],
    [{ Label: 'Goblin' }, 0, 'Goblin'],
  ] as const)('%j seen from seat %s is "%s"', (option, viewer, label) => {
    expect(optionLabel(option, seat, viewer)).toBe(label)
  })

  it.each([
    [{ index: 0, label: 'Draw a card' }, 'Draw a card'],
    [{ index: 2, label: null }, 'Mode 3'],
  ])('%j is "%s"', (mode, label) => {
    expect(modeLabel(mode)).toBe(label)
  })
})

describe('how many modes', () => {
  it.each([
    [1, 1, 'one mode'],
    [2, 2, 'two modes'],
    [0, 2, 'up to two modes'],
    [1, 3, '1 to three modes'],
    [11, 11, '11 modes'],
  ])('from %i to %i is "%s"', (min, max, words) => {
    expect(modeBounds(min, max)).toBe(words)
  })

  it.each([
    [{ min: 1, max: 1, repeat: false, chosen: 0 }, 'Charm: choose one mode.'],
    [{ min: 2, max: 2, repeat: false, chosen: 0 }, 'Charm: choose two modes.'],
    [{ min: 0, max: 2, repeat: false, chosen: 0 }, 'Charm: choose up to two modes, or none.'],
    [
      { min: 3, max: 3, repeat: true, chosen: 0 },
      'Charm: choose three modes, a mode more than once if you like.',
    ],
    [{ min: 2, max: 2, repeat: false, chosen: 1 }, 'Charm: 1 of 2 modes chosen. Choose 1 more.'],
    [
      { min: 1, max: 3, repeat: false, chosen: 1 },
      'Charm: 1 mode chosen. Confirm the modes, or choose up to 2 more.',
    ],
    [
      { min: 2, max: 2, repeat: false, chosen: 2 },
      'Charm: 2 modes chosen. Confirm the modes, or take one back.',
    ],
  ])('%j reads "%s"', (asked, sentence) => {
    expect(modesSentence({ asker: 'Charm', ...asked })).toBe(sentence)
  })
})

describe('pressing a mode', () => {
  it.each([
    ['chooses a mode', [], 4, { max: 2, repeat: false }, [4]],
    ['takes a chosen mode back', [4, 2], 4, { max: 2, repeat: false }, [2]],
    ['changes nothing once the most are chosen', [1, 2], 3, { max: 2, repeat: false }, [1, 2]],
    ['chooses a mode again when it may repeat', [4], 4, { max: 3, repeat: true }, [4, 4]],
    ['adds nothing past the most when it may repeat', [4, 4, 4], 4, { max: 3, repeat: true }, [4, 4, 4]],
  ])('%s', (_what, chosen, index, bounds, expected) => {
    expect(pressedMode(chosen, index, bounds)).toEqual(expected)
  })

  it.each([
    [
      [3, 1],
      [1, 3],
    ],
    [
      [2, 2, 0],
      [0, 2, 2],
    ],
    [[], []],
  ])('answers %j in the order on the card, %j', (chosen, answer) => {
    expect(modesAnswer(chosen)).toEqual(answer)
  })

  it('does not change the list it is given', () => {
    const chosen = [3, 1]
    modesAnswer(chosen)
    pressedMode(chosen, 2, { max: 3, repeat: false })
    expect(chosen).toEqual([3, 1])
  })
})

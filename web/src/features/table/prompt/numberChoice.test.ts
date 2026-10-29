/** Goal: for many ranges (closed, open, with numbers left out, a single value) the number prompt allows exactly the whole numbers from min to max that `except` does not name, steps over the left-out ones, starts on the smallest allowed number, says why a number is refused, and the total cost of an X adds `per_x` generic mana for each 1 of X. */
import { describe, expect, it } from 'vitest'
import type { NumberOptions } from '@/protocol/generated'
import { cost } from '@/test-utils/mana'
import { costWords } from './payText'
import {
  firstAllowed,
  isAllowed,
  numberProblem,
  numberSentence,
  OPEN_MAX,
  rangeWords,
  step,
  totalFor,
} from './numberChoice'

const range = (min: number, max: number | null, except: number[] = []): NumberOptions => ({
  source: 1,
  min,
  max,
  except,
})

describe('allowed numbers', () => {
  it.each<[string, NumberOptions, number[], number[]]>([
    ['0 to 3', range(0, 3), [0, 1, 2, 3], [-1, 4, 1.5]],
    ['2 to 2', range(2, 2), [2], [1, 3]],
    ['0 to 4 without 2', range(0, 4, [2]), [0, 1, 3, 4], [2, 5]],
    ['1 or more', range(1, null), [1, 7, OPEN_MAX], [0, OPEN_MAX + 1]],
  ])('%s', (_what, options, yes, no) => {
    for (const n of yes) expect(isAllowed(options, n)).toBe(true)
    for (const n of no) expect(isAllowed(options, n)).toBe(false)
  })

  it.each<[string, NumberOptions, number, 1 | -1, number]>([
    ['up by one', range(0, 3), 1, 1, 2],
    ['down by one', range(0, 3), 1, -1, 0],
    ['not past the top', range(0, 3), 3, 1, 3],
    ['not below the bottom', range(0, 3), 0, -1, 0],
    ['over a left-out number', range(0, 4, [2]), 1, 1, 3],
    ['back over it', range(0, 4, [2]), 3, -1, 1],
    ['over two left out', range(0, 5, [1, 2]), 0, 1, 3],
    ['nowhere when the rest is left out', range(0, 2, [1, 2]), 0, 1, 0],
  ])('steps %s', (_what, options, from, by, to) => {
    expect(step(options, from, by)).toBe(to)
  })

  it.each<[NumberOptions, number]>([
    [range(0, 3), 0],
    [range(2, 5), 2],
    [range(0, 3, [0]), 1],
    [range(0, 4, [0, 1]), 2],
  ])('starts on the smallest allowed number of %j: %i', (options, first) => {
    expect(firstAllowed(options)).toBe(first)
  })

  it.each<[NumberOptions, number, string | undefined]>([
    [range(0, 3), 2, undefined],
    [range(0, 3), 4, 'Choose 3 or less.'],
    [range(1, 3), 0, 'Choose 1 or more.'],
    [range(0, 3, [2]), 2, '2 is not allowed here.'],
    [range(0, 3), Number.NaN, 'Enter a whole number.'],
    [range(0, 3), 1.5, 'Enter a whole number.'],
  ])('refuses with a reason: %j and %d', (options, n, problem) => {
    expect(numberProblem(options, n)).toBe(problem)
  })
})

describe('words', () => {
  it.each<[NumberOptions, string]>([
    [range(0, 3), 'From 0 to 3.'],
    [range(0, 4, [2]), 'From 0 to 4, except 2.'],
    [range(1, null), '1 or more.'],
  ])('reads the range of %j as "%s"', (options, words) => {
    expect(rangeWords(options)).toBe(words)
  })

  it.each([
    ['Fireball', 'Choose a value for X.', 'Fireball: Choose a value for X.'],
    ['A card', '', 'A card: choose a number.'],
  ])('asks as %s with "%s"', (asker, text, sentence) => {
    expect(numberSentence(asker, text)).toBe(sentence)
  })
})

describe('the total cost of an X', () => {
  it.each<[string, number, number, string]>([
    ['{R}', 1, 0, '1 red'],
    ['{R}', 1, 3, '3 generic and 1 red'],
    ['{1}{R}', 1, 3, '4 generic and 1 red'],
    ['{G}', 2, 2, '4 generic and 1 green'],
    ['', 1, 5, '5 generic'],
    ['{W/U}', 1, 1, '1 generic and 1 white or blue'],
  ])('%s with %i generic per X and X = %i is %s', (at, perX, x, words) => {
    expect(costWords(totalFor(cost(at), perX, x))).toBe(words)
  })
})

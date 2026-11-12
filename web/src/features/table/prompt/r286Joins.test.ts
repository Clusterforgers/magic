import { describe, expect, it } from 'vitest'

import type { JoinRule } from '@/protocol/generated/JoinRule'
import type { ObjectId } from '@/protocol/generated/ObjectId'

import { brokenJoins, joinWords } from './r286Joins'

const id = (n: number): ObjectId => n as unknown as ObjectId
const rule = (object: number, at_least: number, among: number[], alone = false): JoinRule => ({
  object: id(object),
  at_least,
  among: among.map(id),
  alone,
})

describe('join rules of a declaration', () => {
  it.each([
    ['two others joined', rule(1, 2, [2, 3, 4]), [1, 2, 3], 0],
    ['one other of two needed', rule(1, 2, [2, 3, 4]), [1, 2], 1],
    ['an other outside the kind', rule(1, 1, [2]), [1, 3], 1],
    ['an other of the kind', rule(1, 1, [2]), [1, 2], 0],
    ['the creature not declared', rule(1, 1, [2]), [3], 0],
    ['alone and alone', rule(1, 0, [], true), [1], 0],
    ['alone with another', rule(1, 0, [], true), [1, 2], 1],
  ])('%s', (_, held, declared, broken) => {
    expect(brokenJoins([held], declared.map(id))).toHaveLength(broken)
  })

  it('says each rule in words', () => {
    expect(joinWords(rule(1, 2, [2]), 'attack')).toBe(
      "can't attack unless at least 2 other creatures also attack",
    )
    expect(joinWords(rule(1, 1, [2]), 'block')).toBe("can't block unless another creature also blocks")
    expect(joinWords(rule(1, 0, [], true), 'attack')).toBe('can attack only alone')
  })
})

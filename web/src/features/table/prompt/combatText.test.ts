/** Goal: the words of the combat prompts are right for every shape of attack and block: who is attacked and how many attackers go where, one name per creature even when names repeat, how blockers group by attacker, and the keys that tell defenders apart. */
import { describe, expect, it } from 'vitest'
import type { AttackerOption, AttackTarget } from '@/protocol/generated'
import {
  attackSummary,
  blockersByAttacker,
  blockSummary,
  countOf,
  distinctTargets,
  sameTarget,
  targetKey,
  uniqueNames,
} from './combatText'
import type { TargetNames } from './combatText'

const names: TargetNames = {
  short: (target) =>
    'Player' in target ? (['Ana', 'Bo', 'Cy', 'Dee'][target.Player] ?? '?') : `Jace${target.Planeswalker}`,
  long: (target) => ('Player' in target ? names.short(target) : `Jace, Bo's planeswalker`),
}
const at = (...seats: number[]): AttackTarget[] => seats.map((seat) => ({ Player: seat }))

describe('attackSummary', () => {
  it.each([
    ['nothing', [], 'No attackers'],
    ['one attacker', at(1), '1 attacker at Bo'],
    ['three at one seat', at(1, 1, 1), '3 attackers at Bo'],
    ['two seats', at(1, 2, 1), '3 attackers: 2 at Bo, 1 at Cy'],
    ['groups in the order they were chosen', at(3, 1, 3, 2), '4 attackers: 2 at Dee, 1 at Bo, 1 at Cy'],
    ['a planeswalker and a player', [{ Player: 1 }, { Planeswalker: 9 }], '2 attackers: 1 at Bo, 1 at Jace9'],
    ['the same planeswalker twice', [{ Planeswalker: 9 }, { Planeswalker: 9 }], '2 attackers at Jace9'],
  ] as [string, AttackTarget[], string][])('says %s', (_name, targets, expected) => {
    expect(attackSummary(targets, names)).toBe(expected)
  })
})

describe('targets', () => {
  it.each([
    [{ Player: 1 }, 'player:1'],
    [{ Planeswalker: 1 }, 'walker:1'],
  ] as [AttackTarget, string][])('has a key for %j', (target, key) => {
    expect(targetKey(target)).toBe(key)
  })

  it.each([
    [{ Player: 1 }, { Player: 1 }, true],
    [{ Player: 1 }, { Player: 2 }, false],
    [{ Player: 1 }, { Planeswalker: 1 }, false],
    [{ Planeswalker: 3 }, { Planeswalker: 3 }, true],
  ] as [AttackTarget, AttackTarget, boolean][])('compares %j with %j', (a, b, same) => {
    expect(sameTarget(a, b)).toBe(same)
  })

  it('lists every defender once, in the order they first appear', () => {
    const attackers: AttackerOption[] = [
      { object: 1, defenders: [{ Player: 2 }, { Player: 1 }] },
      { object: 2, defenders: [{ Player: 1 }, { Planeswalker: 7 }, { Player: 3 }] },
      { object: 3, defenders: [] },
    ]
    expect(distinctTargets(attackers)).toEqual([
      { Player: 2 },
      { Player: 1 },
      { Planeswalker: 7 },
      { Player: 3 },
    ])
    expect(distinctTargets([])).toEqual([])
  })
})

describe('uniqueNames', () => {
  it.each([
    ['all different', [['Elf'], ['Bear'], ['Wurm']], ['Elf', 'Bear', 'Wurm']],
    ['one pair', [['Elf'], ['Bear'], ['Elf']], ['Elf 1', 'Bear', 'Elf 2']],
    [
      'two pairs and a single',
      [['Elf'], ['Bear'], ['Elf'], ['Bear'], ['Wurm']],
      ['Elf 1', 'Bear 1', 'Elf 2', 'Bear 2', 'Wurm'],
    ],
    ['a token army', [['Goblin'], ['Goblin'], ['Goblin']], ['Goblin 1', 'Goblin 2', 'Goblin 3']],
    ['nothing', [], []],
  ] as [string, [string][], string[]][])('with %s', (_name, given, expected) => {
    const items = given.map(([name], index) => ({ id: index + 1, name }))
    const result = uniqueNames(items)
    expect(items.map((item) => result.get(item.id))).toEqual(expected)
  })
})

describe('blocks', () => {
  it.each([
    [0, 0, 'No blocks'],
    [1, 1, '1 blocker on 1 attacker'],
    [2, 1, '2 blockers on 1 attacker'],
    [5, 3, '5 blockers on 3 attackers'],
  ])('sums %i blockers on %i attackers up as "%s"', (blockers, attackers, expected) => {
    expect(blockSummary(blockers, attackers)).toBe(expected)
  })

  it('groups blockers by the attacker they block, in the order the blocks were made', () => {
    const grouped = blockersByAttacker(
      new Map([
        [10, 1],
        [11, 2],
        [12, 1],
      ]),
    )
    expect([...grouped]).toEqual([
      [1, [10, 12]],
      [2, [11]],
    ])
    expect(blockersByAttacker(new Map()).size).toBe(0)
  })

  it.each([
    [1, 'blocker', undefined, '1 blocker'],
    [3, 'blocker', undefined, '3 blockers'],
    [0, 'attacker', undefined, '0 attackers'],
    [2, 'wolf', 'wolves', '2 wolves'],
  ])('counts %i %s', (count, one, many, expected) => {
    expect(countOf(count, one, many)).toBe(expected)
  })
})

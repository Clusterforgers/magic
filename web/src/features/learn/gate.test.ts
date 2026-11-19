// Goal: the lesson gate lets through exactly the input a step allows, for every decision kind a lesson meets, and refuses every other answer; `narrow` keeps exactly the cards, attackers, blockers and targets the step names and marks the rest of the hand as held for the lesson.
import { describe, expect, it } from 'vitest'
import type { Answer, Decision, ObjectView } from '@/protocol/generated'
import { allowed, narrow, stepFits } from './gate'
import type { HeldOptions } from './gate'
import type { Allow } from './step'

const NAMES: Record<number, string> = {
  1: 'Forest',
  2: 'Grizzly Bears',
  3: 'Llanowar Elves',
  4: 'Serra Angel',
  5: 'Dregscape Zombie',
  6: 'Dregscape Zombie',
  7: 'Giant Growth',
}
const OBJECTS = Object.fromEntries(
  Object.entries(NAMES).map(([id, name]) => [Number(id), { id: Number(id), name } as ObjectView]),
)

const object = (id: number) => ({ Object: { id, zcc: 0 } }) as never

const CASES: [string, Allow, Answer, boolean][] = [
  ['keep', { kind: 'keep' }, { Mulligan: 'Keep' }, true],
  ['a mulligan at keep', { kind: 'keep' }, { Mulligan: 'Mulligan' }, false],
  ['the land', { kind: 'land', name: 'Forest' }, { Priority: { PlayLand: 1 } }, true],
  ['a pass at land', { kind: 'land', name: 'Forest' }, { Priority: 'Pass' }, false],
  ['a spell at land', { kind: 'land', name: 'Forest' }, { Priority: { CastSpell: 2 } }, false],
  ['the spell', { kind: 'cast', name: 'Grizzly Bears' }, { Priority: { CastSpell: 2 } }, true],
  ['another spell', { kind: 'cast', name: 'Grizzly Bears' }, { Priority: { CastSpell: 3 } }, false],
  ['a pass', { kind: 'pass' }, { Priority: 'Pass' }, true],
  ['a land at pass', { kind: 'pass' }, { Priority: { PlayLand: 1 } }, false],
  ['Pay for me', { kind: 'pay', how: 'auto' }, { PayMana: 'Auto' }, true],
  ['a cancel at pay', { kind: 'pay', how: 'auto' }, { PayMana: 'Cancel' }, false],
  [
    'the confirmed proposal of Pay for me',
    { kind: 'pay', how: 'auto' },
    { PayMana: { Manual: { mana: [], life: 0 } as never } },
    true,
  ],
  [
    'a tapped payment',
    { kind: 'pay', how: 'tap' },
    { PayMana: { Manual: { mana: [], life: 0 } as never } },
    true,
  ],
  ['Pay for me at tap', { kind: 'pay', how: 'tap' }, { PayMana: 'Auto' }, false],
  ['an undo', { kind: 'pay', how: 'tap' }, 'Undo', false],
  [
    'the attack',
    { kind: 'attack', names: ['Grizzly Bears'] },
    { DeclareAttackers: [[2, { Player: 1 }]] },
    true,
  ],
  ['no attack', { kind: 'attack', names: ['Grizzly Bears'] }, { DeclareAttackers: [] }, false],
  [
    'too many attackers',
    { kind: 'attack', names: ['Grizzly Bears'] },
    {
      DeclareAttackers: [
        [2, { Player: 1 }],
        [4, { Player: 1 }],
      ],
    },
    false,
  ],
  [
    'the block',
    { kind: 'block', blocker: 'Serra Angel', attacker: 'Dregscape Zombie' },
    { DeclareBlockers: [[4, 6]] },
    true,
  ],
  [
    'no blocks',
    { kind: 'block', blocker: 'Serra Angel', attacker: 'Dregscape Zombie' },
    { DeclareBlockers: [] },
    false,
  ],
  [
    'another blocker',
    { kind: 'block', blocker: 'Serra Angel', attacker: 'Dregscape Zombie' },
    { DeclareBlockers: [[2, 5]] },
    false,
  ],
  [
    'the target',
    { kind: 'target', name: 'Grizzly Bears' },
    { ChooseTargets: { Targets: [object(2)] } },
    true,
  ],
  [
    'another target',
    { kind: 'target', name: 'Grizzly Bears' },
    { ChooseTargets: { Targets: [object(5)] } },
    false,
  ],
  ['a cancelled target', { kind: 'target', name: 'Grizzly Bears' }, { ChooseTargets: 'Cancel' }, false],
  ['the return', { kind: 'choose', yes: true }, { YesNo: true }, true],
  ['no return', { kind: 'choose', yes: true }, { YesNo: false }, false],
  ['an answer at a stop step', { kind: 'stop', side: 'Opponent', step: 'End' }, { Priority: 'Pass' }, false],
  ['an answer at End turn', { kind: 'endTurn' }, { Priority: 'Pass' }, false],
  ['an answer at a look', { kind: 'look' }, { Priority: 'Pass' }, false],
]

describe('allowed', () => {
  it.each(CASES)('%s', (_, allow, answer, want) => {
    expect(allowed(allow, answer, OBJECTS)).toBe(want)
  })

  it('refuses everything while no input is allowed', () => {
    expect(allowed(undefined, { Priority: 'Pass' }, OBJECTS)).toBe(false)
  })
})

const decision = (kind: Decision['kind']): Decision => ({ player: 0, kind }) as Decision

describe('narrow', () => {
  const priority = decision({
    Priority: {
      playable_lands: [1],
      castable: [2, 3],
      mana_abilities: [{ source: { object: 3 } } as never],
      abilities: [{ object: 4, index: 0 } as never],
    } as never,
  })

  it.each<[string, Allow | undefined, number[], number[], number[]]>([
    ['the land', { kind: 'land', name: 'Forest' }, [1], [], [2, 3]],
    ['the spell', { kind: 'cast', name: 'Grizzly Bears' }, [], [2], [1, 3]],
    ['a pass', { kind: 'pass' }, [], [], [1, 2, 3]],
    ['a read step', undefined, [], [], [1, 2, 3]],
  ])('keeps only %s playable and holds the rest', (_, allow, lands, castable, held) => {
    const kind = narrow(priority, allow, OBJECTS).kind
    if (!('Priority' in kind)) throw new Error('not a priority')
    const options = kind.Priority as HeldOptions
    expect(options.playable_lands).toEqual(lands)
    expect(options.castable).toEqual(castable)
    expect([...(options.held ?? [])].toSorted()).toEqual(held)
    expect(options.mana_abilities).toEqual([])
    expect(options.abilities).toEqual([])
  })

  it('offers only the attackers the step names', () => {
    const attack = decision({
      DeclareAttackers: {
        attackers: [
          { object: 2, defenders: [{ Player: 1 }] },
          { object: 4, defenders: [{ Player: 1 }] },
        ],
      },
    })
    const kind = narrow(attack, { kind: 'attack', names: ['Grizzly Bears'] }, OBJECTS).kind
    expect('DeclareAttackers' in kind && kind.DeclareAttackers.attackers.map((each) => each.object)).toEqual([
      2,
    ])
  })

  it('offers only the blocker and the attackers the step names', () => {
    const block = decision({
      DeclareBlockers: {
        blockers: [
          { object: 2, can_block: [5, 6] },
          { object: 4, can_block: [5, 6] },
        ],
      },
    })
    const kind = narrow(
      block,
      { kind: 'block', blocker: 'Serra Angel', attacker: 'Dregscape Zombie' },
      OBJECTS,
    ).kind
    expect('DeclareBlockers' in kind && kind.DeclareBlockers.blockers).toEqual([
      { object: 4, can_block: [5, 6] },
    ])
  })

  it('offers only the target the step names', () => {
    const targets = decision({
      ChooseTargets: { source: 7, spec: 0, legal: [object(2), object(5)], min: 1, max: 1, can_cancel: true },
    })
    const kind = narrow(targets, { kind: 'target', name: 'Grizzly Bears' }, OBJECTS).kind
    expect('ChooseTargets' in kind && kind.ChooseTargets.legal).toEqual([object(2)])
  })

  it('leaves a decision the step does not narrow as it is', () => {
    const yes = decision({ YesNo: { source: 4, commander_return: true } })
    expect(narrow(yes, { kind: 'choose', yes: true }, OBJECTS)).toBe(yes)
  })
})

describe('stepFits', () => {
  it.each<[string, Allow, Decision | null, boolean]>([
    ['a pass at priority', { kind: 'pass' }, decision({ YesNo: { source: 1 } }), false],
    ['a return at a yes or no', { kind: 'choose', yes: true }, decision({ YesNo: { source: 1 } }), true],
    ['a stop while nothing waits', { kind: 'stop', side: 'Opponent', step: 'End' }, null, true],
    ['a pay while nothing waits', { kind: 'pay', how: 'auto' }, null, false],
  ])('%s', (_, allow, pending, want) => {
    expect(stepFits(allow, pending)).toBe(want)
  })
})

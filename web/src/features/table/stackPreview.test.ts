// Goal: the preview of every kind of stack entry (spell, copy, face-down spell, activated, triggered and by-hand ability, ability of a hidden source) says what it resolves, who controls it and what it targets, and draws the ability's source card large, without naming a card the viewer may not see.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { TEXT_NOT_SHOWN, shownObject, stackSummary } from './stackPreview'
import type { StackEntry } from './stackTargets'
import type { TargetNamer } from './stackText'

const names: TargetNamer = { seat: (seat) => ['Ana', 'Bo'][seat] ?? 'Seat', card: () => 'Hill Giant' }
const seat = (id: number): string => names.seat(id)

function object(over: Partial<ObjectView>): ObjectView {
  return {
    id: 1,
    owner: 1,
    controller: 1,
    zone: 'Stack',
    name: 'Fireball',
    mana_cost: null,
    supertypes: [],
    types: [],
    subtypes: [],
    colors: [],
    power: null,
    toughness: null,
    abilities: [],
    permanent: null,
    ...over,
  } as ObjectView
}

const aimed: StackEntry = {
  id: 1,
  controller: 1,
  targets: [
    { key: 'a', target: { Object: { id: 4, counter: 0 } }, gone: false, onStack: false },
    { key: 'b', target: { Player: 0 }, gone: true, onStack: false },
  ],
}
const source = object({ id: 7, name: 'Soul Warden', zone: 'Battlefield', controller: 1 })

describe('the summary of a stack entry', () => {
  it.each([
    ['a spell', object({}), undefined, aimed, 'Fireball, cast by Bo', [], 2],
    ['a copy', object({ copy: {} }), undefined, undefined, 'Copy of Fireball, controlled by Bo', [], 0],
    [
      'a face-down spell',
      object({ name: '', face_down: true }),
      undefined,
      undefined,
      'Face-down spell, cast by Bo',
      ['Its card is face down: only the player who cast it may look at it.'],
      0,
    ],
    [
      'an activated ability',
      object({ kind: 'Ability', source: 7, rules_text: '{T}: Soul Warden deals 1 damage to target player.' }),
      source,
      aimed,
      'Activated ability of Fireball, controlled by Bo',
      ['{T}: Soul Warden deals 1 damage to target player.'],
      2,
    ],
    [
      'a triggered ability',
      object({
        kind: 'Ability',
        source: 7,
        rules_text: 'Whenever another creature enters, you gain 1 life.',
      }),
      source,
      undefined,
      'Triggered ability of Fireball, controlled by Bo',
      ['Whenever another creature enters, you gain 1 life.'],
      0,
    ],
    [
      'a by-hand ability without words',
      object({ kind: 'Ability', source: 7 }),
      object({ id: 7, manual: [{ text: 'Do the thing by hand.' }] as ObjectView['manual'] }),
      undefined,
      'Ability of Fireball, controlled by Bo',
      ['Do the thing by hand.'],
      0,
    ],
    [
      'an ability of a hidden source',
      object({ kind: 'Ability', name: '', source: 7 }),
      undefined,
      undefined,
      'Ability of a face-down permanent, controlled by Bo',
      [TEXT_NOT_SHOWN],
      0,
    ],
  ] as const)('names %s', (_, stacked, from, entry, heading, rules, aims) => {
    const summary = stackSummary(stacked, from, entry, seat, names)
    expect(summary.heading).toBe(heading)
    expect(summary.rules).toEqual(rules)
    expect(summary.targets).toHaveLength(aims)
  })

  it('marks a target that is no longer legal', () => {
    const summary = stackSummary(object({}), undefined, aimed, seat, names)
    expect(summary.targets).toEqual(['Hill Giant', 'Ana (no longer a legal target)'])
  })

  it('never carries the name of a hidden source', () => {
    const hidden = object({ kind: 'Ability', name: '', source: 7 })
    const summary = stackSummary(hidden, undefined, undefined, seat, names)
    expect(JSON.stringify(summary)).not.toContain('Soul Warden')
  })
})

describe('the card drawn large', () => {
  it.each([
    ['an ability with its source in view', object({ kind: 'Ability', source: 7 }), source, 7],
    ['an ability whose source left the view', object({ kind: 'Ability', source: 7 }), undefined, 1],
    ['a spell', object({}), undefined, 1],
    ['a spell that has a source', object({ source: 7 }), source, 1],
  ])('is the card of %s', (_, stacked, from, id) => {
    expect(shownObject(stacked, from).id).toBe(id)
  })
})

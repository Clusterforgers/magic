// Goal: every panel command says what it will change before it is applied, names the change on its button, builds the exact `ManualAction` the protocol takes, and refuses an amount out of range, a change to nothing, more cards than the library holds or more counters than the seat has, in plain words.
import { describe, expect, it } from 'vitest'
import type { ManualAction } from '@/protocol/generated'
import { panelGroups, pressPreset, previewOf, startDraft } from './commands'
import type { Command, CommandId, PanelDraft, PanelInput } from './commands'

const INPUT: PanelInput = {
  seat: 0,
  name: 'Ana',
  life: 40,
  library: 52,
  hand: 4,
  counters: [[{ Other: 'energy' }, 2]],
  looked: [],
}

function command(id: CommandId, input: PanelInput = INPUT): Command {
  const found = panelGroups(input)
    .flatMap((group) => group.commands)
    .find((each) => each.id === id)
  if (found === undefined) throw new Error(`no ${id}`)
  return found
}

const draft = (amount: number, more: Partial<PanelDraft> = {}): PanelDraft => ({
  amount,
  token: 'Treasure',
  mana: 'Green',
  roll: 'd20',
  sides: 10,
  ...more,
})

describe('panelGroups', () => {
  it('lists You, Library and Make in order with their commands', () => {
    expect(panelGroups(INPUT).map((group) => [group.label, group.commands.map((c) => c.id)])).toEqual([
      ['You', ['life', 'poison', 'energy', 'experience']],
      ['Library', ['draw', 'look', 'mill', 'exileTop', 'bottomTop', 'shuffle', 'revealTop']],
      ['Make', ['token', 'mana', 'roll']],
    ])
  })

  it('gives every command a label and one line of what it does', () => {
    for (const each of panelGroups(INPUT).flatMap((group) => group.commands)) {
      expect(each.label).not.toBe('')
      expect(each.effect).toMatch(/^[A-Z].*\.$/)
    }
  })

  it('makes every command inert with the reason once the seat is out', () => {
    const off = 'The game is over.'
    for (const each of panelGroups({ ...INPUT, off }).flatMap((group) => group.commands)) {
      expect(each.unavailable).toBe(off)
    }
  })

  it('makes the library commands inert on an empty library', () => {
    const empty = { ...INPUT, library: 0 }
    expect(command('draw', empty).unavailable).toBe('Your library is empty.')
    expect(command('mill', empty).unavailable).toBe('Your library is empty.')
    expect(command('shuffle', empty).unavailable).toBeUndefined()
  })
})

describe('previewOf', () => {
  it.each<[CommandId, PanelDraft, string, string, ManualAction]>([
    ['life', draft(-5), 'Ana: 40 → 35 life', 'Lose 5 life', { Life: { player: 0, delta: -5 } }],
    ['life', draft(3), 'Ana: 40 → 43 life', 'Gain 3 life', { Life: { player: 0, delta: 3 } }],
    [
      'poison',
      draft(1),
      'Ana: 0 → 1 poison',
      'Add 1 poison counter',
      { PlayerCounters: { player: 0, counter: 'poison', delta: 1 } },
    ],
    [
      'energy',
      draft(-2),
      'Ana: 2 → 0 energy',
      'Remove 2 energy counters',
      { PlayerCounters: { player: 0, counter: { Other: 'energy' }, delta: -2 } },
    ],
    ['draw', draft(2), 'Ana: library 52 → 50, hand 4 → 6', 'Draw 2 cards', { Draw: { count: 2 } }],
    [
      'mill',
      draft(3),
      'Ana: the top 3 cards go to your graveyard (library 52 → 49)',
      'Mill 3 cards',
      { MoveTop: { count: 3, to: 'Graveyard' } },
    ],
    [
      'exileTop',
      draft(1),
      'Ana: the top card goes to exile (library 52 → 51)',
      'Exile 1 card',
      { MoveTop: { count: 1, to: 'Exile' } },
    ],
    [
      'bottomTop',
      draft(2),
      'Ana: the top 2 cards go to the bottom of your library',
      'Put 2 cards on the bottom',
      { MoveTop: { count: 2, to: 'LibraryBottom' } },
    ],
    ['shuffle', draft(0), 'Ana: 52 cards in a new random order', 'Shuffle your library', { Shuffle: {} }],
    [
      'token',
      draft(2, { token: 'Soldier' }),
      '2 Soldier tokens enter under your control',
      'Create 2 Soldiers',
      { CreateToken: { token: { Common: 'Soldier' }, count: 2 } },
    ],
    [
      'mana',
      draft(2),
      'Ana: {G}{G} into your mana pool until the step ends',
      'Add {G}{G}',
      { AddMana: { mana: 'Green', count: 2 } },
    ],
  ])('%s with %o says %s, applies as %s', (id, chosen, preview, apply, action) => {
    expect(previewOf(command(id), chosen, INPUT)).toEqual({ preview, apply, action })
  })

  it.each<[CommandId, PanelDraft, string]>([
    ['life', draft(0), 'Choose an amount with the buttons or the field.'],
    ['life', draft(1001), 'Choose a change from -1000 to 1000.'],
    ['poison', draft(-1), 'You have 0 poison counters.'],
    ['draw', draft(0), 'Choose 1 to 10.'],
    ['draw', draft(11), 'Choose 1 to 10.'],
    ['draw', draft(1.5), 'Choose 1 to 10.'],
    ['mill', draft(5), 'Your library has 4 cards.'],
    ['token', draft(11), 'Choose 1 to 10.'],
    ['mana', draft(21), 'Choose 1 to 20.'],
  ])('%s with %o is refused: %s', (id, chosen, problem) => {
    const input = id === 'mill' ? { ...INPUT, library: 4 } : INPUT
    expect(previewOf(command(id, input), chosen, input)).toEqual({ problem })
  })

  it('refuses an inert command with its reason', () => {
    const off = { ...INPUT, off: 'The game is over.' }
    expect(previewOf(command('life', off), draft(1), off)).toEqual({ problem: 'The game is over.' })
  })
})

describe('pressPreset and startDraft', () => {
  it.each<[CommandId, number, number, number]>([
    ['life', 0, -5, -5],
    ['life', -5, -1, -6],
    ['life', -999, -5, -1000],
    ['draw', 1, 3, 3],
    ['draw', 3, 1, 1],
  ])('%s at %i pressed %i is %i', (id, amount, preset, expected) => {
    const spec = command(id).amount
    if (spec === undefined) throw new Error('no amount')
    expect(pressPreset(spec, amount, preset)).toBe(expected)
  })

  it('opens a signed command at no change and a count at one', () => {
    expect(startDraft(command('life')).amount).toBe(0)
    expect(startDraft(command('draw')).amount).toBe(1)
    expect(startDraft(command('shuffle')).amount).toBe(0)
  })
})

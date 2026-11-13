// Goal: the Delves deck choice lists the player's Commander decks under their deck groups (the last used group first, Ungrouped after the groups, "Your decks" without groups), then the precons, and remembers the group of a deck chosen.
import { afterEach, describe, expect, it } from 'vitest'
import { deckItem } from '@/features/decks/fakeDecks'
import { groupView } from '@/features/decks/groups/fakeGroups'
import { lastGroup } from '@/features/decks/groups/lastGroup'
import { groupedOptions, optionGroups, rememberChoice } from './words'

afterEach(() => localStorage.clear())

const DECKS = [
  deckItem({ id: 'a', name: 'Atraxa' }),
  deckItem({ id: 'b', name: 'Burn', format: 'modern' }),
  deckItem({ id: 'k', name: 'Krenko' }),
  deckItem({ id: 'm', name: 'Muldrotha' }),
]
const GROUPS = [groupView('fun', 'Fun', ['k']), groupView('hard', 'Hardcore', ['m', 'b'])]
const PRECONS = [{ code: 'c19', name: 'Merciless Rage' }]

const headings = (last: string | null, groups = GROUPS) =>
  optionGroups(groupedOptions(DECKS, groups, PRECONS, last)).map(([group, options]) => [
    group,
    options.map((o) => o.label),
  ])

describe('the Delves deck choice', () => {
  it.each([
    [
      null,
      GROUPS,
      [
        ['Fun', ['Krenko']],
        ['Hardcore', ['Muldrotha']],
        ['Ungrouped', ['Atraxa']],
      ],
    ],
    [
      'hard',
      GROUPS,
      [
        ['Hardcore', ['Muldrotha']],
        ['Fun', ['Krenko']],
        ['Ungrouped', ['Atraxa']],
      ],
    ],
    [null, [], [['Your decks', ['Atraxa', 'Krenko', 'Muldrotha']]]],
  ] as const)('with the last group %s lists %j', (last, groups, mine) => {
    expect(headings(last, [...groups])).toEqual([...mine, ['Precons', ['Merciless Rage']]])
  })

  it('remembers the group of an own deck chosen and leaves it for a precon', () => {
    const options = groupedOptions(DECKS, GROUPS, PRECONS, null)
    rememberChoice(options, 'k')
    expect(lastGroup()).toBe('fun')
    rememberChoice(options, 'precon:c19')
    expect(lastGroup()).toBe('fun')
    rememberChoice(options, 'a')
    expect(lastGroup()).toBeNull()
  })
})

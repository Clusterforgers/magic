// Goal: the Delves deck picker lists the player's Commander decks in their deck groups (the last used group first, Ungrouped after, one "Your decks" section without groups) and the precons last, narrows them by search and color, leads with the decks last played on a delve, words each deck's validity and keeps the last delves in storage that may fail.
import { describe, expect, it } from 'vitest'
import { deckValidity, notePlayed, pickerSections, PLAYED_KEY, readPlayed } from './deckPicker'
import { PICKER_ENTRIES, PICKER_GROUPS } from './fakePickerDecks'

const titles = (...args: Parameters<typeof pickerSections>) =>
  pickerSections(...args).map((s) => [s.title, s.decks.map((d) => d.name)])

const ALL = { query: '', colours: [] }

describe('pickerSections', () => {
  it.each([
    [
      'groups in order, then Ungrouped and the precons',
      PICKER_GROUPS,
      null,
      [
        ['Fun', ['Goblin Swarm']],
        ['Hardcore', ['Meren Graveyard']],
        ['Ungrouped', ['Atraxa Counters']],
        ['Precons', ['Draconic Dissent', 'Elven Council', 'Riders of Rohan']],
      ],
    ],
    [
      'the last used group first',
      PICKER_GROUPS,
      'hard',
      [
        ['Hardcore', ['Meren Graveyard']],
        ['Fun', ['Goblin Swarm']],
        ['Ungrouped', ['Atraxa Counters']],
        ['Precons', ['Draconic Dissent', 'Elven Council', 'Riders of Rohan']],
      ],
    ],
    [
      'one section of the player’s decks without groups',
      [],
      null,
      [
        ['Your decks', ['Atraxa Counters', 'Goblin Swarm', 'Meren Graveyard']],
        ['Precons', ['Draconic Dissent', 'Elven Council', 'Riders of Rohan']],
      ],
    ],
  ])('lists %s', (_, groups, last, expected) => {
    expect(titles(PICKER_ENTRIES, groups, ALL, {}, last)).toEqual(expected)
  })

  it.each([
    [{ query: 'krenko', colours: [] }, [['Fun', ['Goblin Swarm']]]],
    [{ query: 'rohan', colours: [] }, [['Precons', ['Riders of Rohan']]]],
    [
      { query: '', colours: ['B'] },
      [
        ['Hardcore', ['Meren Graveyard']],
        ['Ungrouped', ['Atraxa Counters']],
        ['Precons', ['Draconic Dissent']],
      ],
    ],
    [{ query: '', colours: ['R', 'W'] }, [['Precons', ['Riders of Rohan']]]],
    [{ query: 'nothing at all', colours: [] }, []],
  ])('narrows to %j', (filter, expected) => {
    expect(titles(PICKER_ENTRIES, PICKER_GROUPS, filter)).toEqual(expected)
  })

  it('leads each section with the decks last played on a delve', () => {
    const played = { 'precon:ltc': '2026-11-10T10:00:00Z', 'precon:x': '2026-11-01T10:00:00Z' }
    expect(titles(PICKER_ENTRIES, [], ALL, played).at(-1)).toEqual([
      'Precons',
      ['Riders of Rohan', 'Elven Council', 'Draconic Dissent'],
    ])
  })
})

describe('deckValidity', () => {
  it.each([
    [PICKER_ENTRIES[0]!, 'ok', 'Legal'],
    [PICKER_ENTRIES[2]!, 'bad', '2 rule issues'],
    [{ ...PICKER_ENTRIES[0]!, byHand: 3 }, 'warn', 'Legal · 3 by hand'],
    [{ ...PICKER_ENTRIES[0]!, state: 'unchecked' as const }, 'warn', 'Not checked yet'],
  ])('words %s', (entry, tone, label) => {
    expect(deckValidity(entry)).toEqual({ tone, label })
  })
})

describe('the last delves', () => {
  const memory = () => {
    const data = new Map<string, string>()
    return {
      getItem: (key: string) => data.get(key) ?? null,
      setItem: (key: string, value: string) => void data.set(key, value),
      removeItem: (key: string) => void data.delete(key),
    }
  }

  it('records and reads a deck’s last delve', () => {
    const store = memory()
    notePlayed('d-krenko', new Date('2026-11-12T08:00:00Z'), store)
    expect(readPlayed(store)).toEqual({ 'd-krenko': '2026-11-12T08:00:00.000Z' })
  })

  it.each([
    ['garbage', 'not json'],
    ['a list', '[1, 2]'],
    ['a number', '7'],
  ])('reads %s as nothing played', (_, raw) => {
    const store = memory()
    store.setItem(PLAYED_KEY, raw)
    expect(readPlayed(store)).toEqual({})
  })

  it('works without storage', () => {
    const broken = {
      getItem: () => {
        throw new Error('blocked')
      },
      setItem: () => {
        throw new Error('blocked')
      },
      removeItem: () => undefined,
    }
    expect(readPlayed(broken)).toEqual({})
    expect(notePlayed('d', new Date('2026-11-12T08:00:00Z'), broken)).toEqual({
      d: '2026-11-12T08:00:00.000Z',
    })
  })
})

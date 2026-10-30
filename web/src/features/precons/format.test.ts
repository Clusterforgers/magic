// Goal: the precon browser's pure rules hold over tables: the status a deck gets from the server's state and `playable` flag and the words of its badge, the release date and commander lines, the search and the set and state filters (accents, several words, newest-first sets), the count line, and the copyable list, which holds one `Name: clause` line for each card that is not Verified and nothing else.
import { describe, expect, it } from 'vitest'
import { playRequest } from './play'
import { board, cardLine, checked, preconItem, preconView } from './fakePrecons'
import {
  checkLine,
  commandersText,
  copyList,
  countLine,
  filterPrecons,
  filtersActive,
  NO_FILTERS,
  releaseText,
  setOptions,
  statusOf,
  statusWords,
} from './format'
import type { Filters, PreconStatus } from './format'

const deck = (name: string, over = {}) =>
  preconItem({ name, code: name.toLowerCase().replace(/\W+/g, '-'), ...over })

describe('statusOf and statusWords', () => {
  it.each<[string, ReturnType<typeof preconItem>, PreconStatus, string]>([
    ['a clean deck', checked(preconItem()), 'whole', 'Whole deck runs'],
    ['one card by hand', checked(preconItem(), { manual: 1 }), 'hand', '1 card played by hand'],
    [
      'partial, manual and stale copies add up',
      checked(preconItem(), { partial: 2, manual: 3, stale: 1 }),
      'hand',
      '6 cards played by hand',
    ],
    ['an unplayable card', checked(preconItem(), { unplayable: 1 }), 'blocked', 'Cannot be taken'],
    [
      'a deck the engine refuses though its check is clean of unplayable cards',
      checked(preconItem(), { manual: 2 }, false),
      'blocked',
      'Cannot be taken',
    ],
  ])('%s', (_, item, status, words) => {
    expect(statusOf(item)).toBe(status)
    expect(statusWords(item)).toBe(words)
  })

  it('says what the check found beside the badge', () => {
    expect(checkLine(checked(preconItem(), { partial: 2, manual: 3 }))).toBe(
      '95 of 100 cards verified; 5 played by hand',
    )
    expect(checkLine(preconItem())).toBe('100 of 100 cards verified')
  })
})

describe('the words of a deck', () => {
  it.each([
    ['2018-10-05', 'Oct 2018'],
    ['2024-06-14', 'Jun 2024'],
    [null, 'Release date unknown'],
    ['soon', 'Release date unknown'],
    ['2024-13-01', 'Release date unknown'],
  ])('releaseText(%s) is %s', (date, words) => {
    expect(releaseText(date)).toBe(words)
  })

  it.each([
    [[], 'No commander'],
    [['Atraxa, Praetors’ Voice'], 'Atraxa, Praetors’ Voice'],
    [['Tymna the Weaver', 'Thrasios, Triton Hero'], 'Tymna the Weaver and Thrasios, Triton Hero'],
    [['A', 'B', 'C'], 'A, B and C'],
  ])('commandersText(%j) is %s', (names, words) => {
    expect(commandersText(names)).toBe(words)
  })

  it.each([
    [3, 3, '3 precons'],
    [1, 1, '1 precon'],
    [0, 12, '0 of 12 precons'],
    [4, 198, '4 of 198 precons'],
  ])('countLine(%i, %i) is %s', (shown, total, words) => {
    expect(countLine(shown, total)).toBe(words)
  })
})

describe('filterPrecons', () => {
  const list = [
    checked(deck('Eldrazi Unbound', { set: 'M3C', commanders: ['Ulalek, Fused Atrocity'] }), { manual: 1 }),
    deck('Tricky Terrain', { set: 'M3C', commanders: ['Satya, Aetherflux Genius'] }),
    checked(deck('Arcane Maelstrom', { set: 'C20', commanders: ['Kalamax, the Stormsire'] }), {
      unplayable: 1,
    }),
    deck('Séance Spirits', { set: 'C18', commanders: ['Anje Falkenrath'] }),
  ]
  const names = (filters: Partial<Filters>): string[] =>
    filterPrecons(list, { ...NO_FILTERS, ...filters }).map((item) => item.name)

  it.each<[string, Partial<Filters>, string[]]>([
    [
      'no filter keeps every deck in order',
      {},
      ['Eldrazi Unbound', 'Tricky Terrain', 'Arcane Maelstrom', 'Séance Spirits'],
    ],
    ['the name, in any case', { text: 'ELDRAZI' }, ['Eldrazi Unbound']],
    ['a commander', { text: 'kalamax' }, ['Arcane Maelstrom']],
    ['the set code', { text: 'c18' }, ['Séance Spirits']],
    ['several words, each anywhere', { text: 'tricky satya' }, ['Tricky Terrain']],
    ['accents are ignored', { text: 'seance' }, ['Séance Spirits']],
    ['spaces around the text are ignored', { text: '  terrain  ' }, ['Tricky Terrain']],
    ['a set', { set: 'M3C' }, ['Eldrazi Unbound', 'Tricky Terrain']],
    ['a state', { status: 'hand' }, ['Eldrazi Unbound']],
    ['the blocked state', { status: 'blocked' }, ['Arcane Maelstrom']],
    ['the whole state', { status: 'whole' }, ['Tricky Terrain', 'Séance Spirits']],
    ['a set and a state together', { set: 'M3C', status: 'whole' }, ['Tricky Terrain']],
    ['nothing matches', { text: 'zzz' }, []],
  ])('%s', (_, filters, expected) => {
    expect(names(filters)).toEqual(expected)
  })

  it('lists the sets in the order they arrive, with their counts', () => {
    expect(setOptions(list)).toEqual([
      { set: 'M3C', count: 2 },
      { set: 'C20', count: 1 },
      { set: 'C18', count: 1 },
    ])
  })

  it.each([
    [NO_FILTERS, false],
    [{ ...NO_FILTERS, text: '   ' }, false],
    [{ ...NO_FILTERS, text: 'a' }, true],
    [{ ...NO_FILTERS, set: 'C18' }, true],
    [{ ...NO_FILTERS, status: 'whole' as const }, true],
  ])('filtersActive(%j) is %s', (filters, active) => {
    expect(filtersActive(filters)).toBe(active)
  })
})

describe('copyList', () => {
  const texts = new Map([['oracle-kozilek', ['Draw two cards. Discard two cards.']]])
  const clause = (text: string, start: number, end: number) => ({ face: 0, span: { start, end }, text })

  it('writes a block for each card that is not Verified, in board order, quoting each clause once the texts are known', () => {
    const view = preconView(preconItem({ name: 'Eldrazi Unbound' }), undefined, {
      boards: [
        board('commander', [cardLine('Ulalek')]),
        board('main', [
          cardLine('Sol Ring'),
          cardLine('Kozilek', 'partial', {
            reasons: [
              clause('the draw is not implemented', 0, 15),
              clause('the discard is not implemented', 16, 34),
            ],
          }),
          cardLine('Thought-Knot Seer', 'manual'),
        ]),
        board('sideboard', [cardLine('Old Card', 'stale'), cardLine('Digital Card', 'unplayable')]),
      ],
    })
    expect(copyList(view, texts).split('\n\n')).toEqual([
      [
        'Kozilek (Partial) - decks: Eldrazi Unbound',
        '  - "Draw two cards.": The draw is not implemented',
        '  - "Discard two cards.": The discard is not implemented',
      ].join('\n'),
      'Thought-Knot Seer (Manual) - decks: Eldrazi Unbound\n  - Played by hand',
      'Old Card (Stale) - decks: Eldrazi Unbound\n  - Its text changed since the rules were written, so it is played by hand',
      'Digital Card (Unplayable) - decks: Eldrazi Unbound\n  - The game cannot play it',
    ])
  })

  it('writes the reasons alone while no text is known', () => {
    const view = preconView(preconItem({ name: 'Eldrazi Unbound' }), [
      cardLine('Kozilek', 'partial', { reasons: [clause('the draw is not implemented', 0, 14)] }),
    ])
    expect(copyList(view)).toBe('Kozilek (Partial) - decks: Eldrazi Unbound\n  - The draw is not implemented')
  })

  it('names the face when a clause is not on the front', () => {
    const view = preconView(preconItem({ name: 'Eldrazi Unbound' }), [
      cardLine('Delver', 'partial', { reasons: [{ face: 1, span: { start: 0, end: 4 }, text: 'it flips' }] }),
    ])
    expect(copyList(view, new Map([['oracle-delver', ['Front', 'Flip it over']]]))).toBe(
      'Delver (Partial) - decks: Eldrazi Unbound\n  - "Flip": Back face: it flips',
    )
  })

  it('is empty when every card is Verified', () => {
    expect(copyList(preconView(preconItem(), [cardLine('Forest')]))).toBe('')
  })
})

describe('playRequest', () => {
  it.each([
    ['a deck with a commander', 'commander', 'commander'],
    ['a deck without one', 'freeform', 'normal'],
  ])('%s makes a %s game when the check format is %s', (_, format, game) => {
    expect(playRequest({ code: 'c18-adaptive-enchantment', format }, '  Ada_L ')).toEqual({
      format: game,
      seats: [{ name: 'Ada_L', deck: 'precon:c18-adaptive-enchantment' }, { name: 'Player 2' }],
      visibility: 'public',
      held: 1,
    })
  })
})

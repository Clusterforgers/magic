// Goal: the menu offers Reveal for a card in the seat's own hand and a note for a card of the seat in a zone every seat sees, sends the reveal as it is, asks for the text of a note, lists both inert with the reason while nothing can be done by hand, and builds a note trimmed, cleared with an empty text, and refused when empty, unchanged, too long or on several lines.
import { describe, expect, it } from 'vitest'
import type { ManualAction, ObjectView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { buildAction, EMPTY_DRAFT, manualMenu } from './offers'
import type { GroupId, MenuInput } from './offers'

const SEAT = 0

function input(object: ObjectView, over: Partial<MenuInput> = {}): MenuInput {
  return {
    seat: SEAT,
    object,
    players: [0, 1],
    hasCommandZone: true,
    library: 40,
    counters: [],
    ownerName: 'Bo',
    ...over,
  }
}

const entries = (menu: ReturnType<typeof manualMenu>, group: GroupId) =>
  menu.groups.find((each) => each.id === group)?.items ?? []

describe('the Show group', () => {
  const inHand = card({ id: 3, zone: { Hand: SEAT }, permanent: null })
  const cases: [string, ObjectView, string[], string[]][] = [
    ['a card in the seat hand', inHand, ['reveal'], ['Reveal to the table']],
    ['a creature on the battlefield', card({ id: 3 }), ['note'], ['Add a note…']],
    ['a creature that has a note', card({ id: 3, note: 'has haste' }), ['note'], ['Change the note…']],
    [
      'a card in the graveyard',
      card({ id: 3, zone: { Graveyard: SEAT }, permanent: null }),
      ['note'],
      ['Add a note…'],
    ],
    ['a card in the library', card({ id: 3, zone: { Library: SEAT }, permanent: null }), [], []],
    ['a card of another seat', card({ id: 3, owner: 1, controller: 1 }), [], []],
    [
      'a hand card of another seat',
      card({ id: 3, owner: 1, controller: 1, zone: { Hand: 1 }, permanent: null }),
      [],
      [],
    ],
  ]
  it.each(cases)('%s offers %j', (_name, object, keys, labels) => {
    const items = entries(manualMenu(input(object)), 'show')
    expect(items.map((each) => each.key)).toEqual(keys)
    expect(items.map((each) => each.label)).toEqual(labels)
  })

  it('sends the reveal as it is and asks for the text of a note', () => {
    expect(entries(manualMenu(input(inHand)), 'show')[0]?.action).toEqual({
      Reveal: { what: { Hand: { object: 3 } } },
    })
    expect(entries(manualMenu(input(card({ id: 3 }))), 'show')[0]?.ask).toBe('note')
  })

  it('lists the entries inert, with the reason, while nothing can be done by hand', () => {
    const menu = manualMenu(input(card({ id: 3 }), { off: 'The game is over.' }))
    expect(entries(menu, 'show')[0]?.unavailable).toBe('The game is over.')
  })
})

describe('the form of a note', () => {
  const note = (text: string, over: Partial<ObjectView> = {}) =>
    buildAction('note', { ...input(card({ id: 3, ...over })), draft: { ...EMPTY_DRAFT, text } })
  it.each<[string, string, Partial<ObjectView>, ManualAction | string]>([
    ['a note', '  has haste ', {}, { Note: { object: 3, text: 'has haste' } }],
    ['a note cleared', '   ', { note: 'old' }, { Note: { object: 3, text: '' } }],
    ['no note to write', '', {}, 'Write a note.'],
    ['the same note', 'old', { note: 'old' }, 'The note is the same.'],
    ['a note that is too long', 'x'.repeat(81), {}, 'Write at most 80 characters.'],
    ['a note with a line break', 'one\ntwo', {}, 'Write the note on one line.'],
  ])('builds %s', (_name, text, over, expected) => {
    const result = note(text, over)
    if (typeof expected === 'string') expect(result).toEqual({ problem: expected })
    else expect(result).toEqual({ action: expected })
  })
})

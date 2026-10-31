// Goal: the doors of a Room are named by the card's printed faces when they are known, else by the object's name (both halves of the whole card, or the one door that is unlocked), else by their place; the title of a Room with both doors locked is its doors as the whole card names them, and the words say each door's name and state.
import { describe, expect, it } from 'vitest'
import { permanent } from '@/test-utils/cards'
import { doorsNamed, doorsOf, doorWord, doorWords, roomTitle } from './rooms'

const ENTRYWAY = 'Grand Entryway'
const ROTUNDA = 'Elegant Rotunda'

describe('doorsNamed', () => {
  it.each<[string, string, [boolean, boolean], string | undefined, string | undefined, [string, string]]>([
    ['the printed faces', '', [false, false], ENTRYWAY, ROTUNDA, [ENTRYWAY, ROTUNDA]],
    ['the faces over the name', ENTRYWAY, [true, false], ENTRYWAY, ROTUNDA, [ENTRYWAY, ROTUNDA]],
    ['the whole card', `${ENTRYWAY} // ${ROTUNDA}`, [true, true], undefined, undefined, [ENTRYWAY, ROTUNDA]],
    ['the first door alone', ENTRYWAY, [true, false], undefined, undefined, [ENTRYWAY, 'Second door']],
    ['the second door alone', ROTUNDA, [false, true], undefined, undefined, ['First door', ROTUNDA]],
    ['no name and no faces', '', [false, false], undefined, undefined, ['First door', 'Second door']],
    ['one face only', '', [false, false], ENTRYWAY, undefined, ['First door', 'Second door']],
  ])('names the doors by %s', (_case, name, unlocked, first, second, expected) => {
    expect(doorsNamed(name, unlocked, first, second).map((door) => door.name)).toEqual(expected)
  })

  it.each<[[boolean, boolean]]>([[[false, false]], [[true, false]], [[false, true]], [[true, true]]])(
    'keeps which doors are unlocked: %j',
    (unlocked) => {
      expect(doorsNamed('', unlocked).map((door) => door.unlocked)).toEqual(unlocked)
    },
  )
})

describe('doorsOf', () => {
  it('is none for a permanent that is not a Room', () => {
    expect(doorsOf(permanent({}))).toBeUndefined()
  })

  it('reads the doors of a Room permanent', () => {
    const room = permanent({ doors: [true, false] }, { name: ENTRYWAY })
    expect(doorsOf(room, [ENTRYWAY, ROTUNDA])).toEqual([
      { name: ENTRYWAY, unlocked: true },
      { name: ROTUNDA, unlocked: false },
    ])
  })
})

describe('roomTitle', () => {
  const locked = doorsNamed('', [false, false], ENTRYWAY, ROTUNDA)
  const unnamed = doorsNamed('', [false, false])
  it.each<[string, string, ReturnType<typeof doorsNamed> | undefined, string]>([
    ['a named permanent', ENTRYWAY, locked, ENTRYWAY],
    ['a permanent that is not a Room', '', undefined, ''],
    ['both doors locked, the faces known', '', locked, `${ENTRYWAY} // ${ROTUNDA}`],
    ['both doors locked, the faces unknown', '', unnamed, 'Room'],
  ])('titles %s', (_case, name, doors, expected) => {
    expect(roomTitle(name, doors)).toBe(expected)
  })
})

describe('the words of the doors', () => {
  const doors = doorsNamed('', [true, false], ENTRYWAY, ROTUNDA)
  it('says a door and its state', () => {
    expect(doors.map(doorWord)).toEqual([`${ENTRYWAY} unlocked`, `${ROTUNDA} locked`])
  })

  it('says both for a screen reader', () => {
    expect(doorWords(doors)).toBe(`Room, ${ENTRYWAY} unlocked, ${ROTUNDA} locked`)
  })
})

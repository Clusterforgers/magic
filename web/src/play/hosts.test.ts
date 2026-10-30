// Goal: the host of an attachment reads from its view (a permanent, a player or nothing), two hosts compare by what they are, an Equipment or Fortification on no host is unattached, and the log keeps where the attachment of an attach event hung before its batch, following the batch's own earlier events and nothing for any other event.
import { describe, expect, it } from 'vitest'
import type { Event, EventView, ObjectView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { hostOf, hostsBefore, isUnattached, sameHost } from './hosts'
import type { HostRef } from './hosts'
import { appendLog } from './store'

const pub = (event: Event): EventView => ({ Public: event })
const objectsOf = (...objects: ObjectView[]) => Object.fromEntries(objects.map((o) => [o.id, o]))

describe('the host of an attachment', () => {
  it.each([
    ['a permanent', { attached_to: 4 }, { card: 4 }],
    ['a player', { attached_player: 1 }, { player: 1 }],
    ['nothing', {}, null],
  ])('reads %s from the view', (_name, over, host) => {
    expect(hostOf(card({ id: 9, ...over }))).toEqual(host)
  })

  it('reads nothing from an object that is not in the view', () => {
    expect(hostOf(undefined)).toBeNull()
  })

  it.each<[string, HostRef | null, HostRef | null, boolean]>([
    ['one permanent', { card: 1 }, { card: 1 }, true],
    ['two permanents', { card: 1 }, { card: 2 }, false],
    ['one player', { player: 1 }, { player: 1 }, true],
    ['a permanent and a player of the same number', { card: 1 }, { player: 1 }, false],
    ['two missing hosts', null, null, true],
    ['a host and none', { card: 1 }, null, false],
  ])('compares %s', (_name, a, b, same) => {
    expect(sameHost(a, b)).toBe(same)
    expect(sameHost(b, a)).toBe(same)
  })

  it.each([
    ['an Equipment on no host', { subtypes: ['Equipment'] }, true],
    ['a Fortification on no host', { subtypes: ['Fortification'] }, true],
    ['an Equipment on a creature', { subtypes: ['Equipment'], attached_to: 3 }, false],
    ['an Equipment in a graveyard', { subtypes: ['Equipment'], zone: { Graveyard: 0 } }, false],
    ['an Aura on no host', { subtypes: ['Aura'] }, false],
    ['a creature', { subtypes: ['Bear'] }, false],
  ] as const)('%s is unattached: %s', (_name, over, unattached) => {
    expect(isUnattached(card({ ...over } as unknown as Partial<ObjectView>))).toBe(unattached)
  })
})

describe('where an attachment hung before an attach event', () => {
  const before = objectsOf(
    card({ id: 7, attached_to: 1 }),
    card({ id: 8, attached_player: 2 }),
    card({ id: 9 }),
  )

  it.each<[string, Event, HostRef | null]>([
    ['a creature to another', { Attached: { object: 7, to: 2 } }, { card: 1 }],
    ['a player to a creature', { Attached: { object: 8, to: 2 } }, { player: 2 }],
    ['nothing to a creature', { Attached: { object: 9, to: 2 } }, null],
    ['a creature to a player', { AttachedToPlayer: { object: 7, player: 0 } }, { card: 1 }],
    ['an object the viewer did not know to a creature', { Attached: { object: 99, to: 2 } }, null],
  ])('reads %s', (_name, event, was) => {
    expect(hostsBefore([pub(event)], before)).toEqual([was])
  })

  it('gives nothing for every other event, hidden ones and bare ones included', () => {
    const events = [
      pub({ Unattached: { object: 7 } }),
      pub({ LifeLost: { player: 0, amount: 1 } }),
      pub('MulliganPhaseBegan'),
      { Hidden: { CardDrawn: { player: 1 } } },
    ] satisfies EventView[]
    expect(hostsBefore(events, before)).toEqual([undefined, undefined, undefined, undefined])
  })

  it('follows the attach events of the batch itself', () => {
    const events = [
      pub({ Attached: { object: 7, to: 2 } }),
      pub({ Attached: { object: 7, to: 3 } }),
      pub({ Unattached: { object: 7 } }),
      pub({ Attached: { object: 7, to: 4 } }),
    ]
    expect(hostsBefore(events, before)).toEqual([{ card: 1 }, { card: 2 }, undefined, null])
  })

  it('is kept on the log entries of a batch that has the objects, and left off without them', () => {
    const events = [pub({ Attached: { object: 7, to: 2 } }), pub({ LifeLost: { player: 0, amount: 1 } })]
    expect(appendLog([], 10, events, before)).toEqual([
      { seq: 10, event: events[0], hostBefore: { card: 1 } },
      { seq: 11, event: events[1] },
    ])
    expect(appendLog([], 10, events)).toEqual([
      { seq: 10, event: events[0] },
      { seq: 11, event: events[1] },
    ])
  })
})

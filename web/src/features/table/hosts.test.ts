// Goal: the host of an attachment reads from its view (a permanent, a player or nothing), two hosts compare by what they are, and the events that attach, unattach, enter or move an object give the host it has afterwards and no other event gives any.
import { describe, expect, it } from 'vitest'
import type { Event, EventView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { hostChangeOf, hostOf, sameHost } from './hosts'
import type { HostRef } from './hosts'

const pub = (event: Event): EventView => ({ Public: event })

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
})

describe('the host change an event tells', () => {
  it.each<[string, EventView, { object: number; to: HostRef | null } | null]>([
    ['Attached', pub({ Attached: { object: 3, to: 1 } }), { object: 3, to: { card: 1 } }],
    [
      'AttachedToPlayer',
      pub({ AttachedToPlayer: { object: 3, player: 2 } }),
      { object: 3, to: { player: 2 } },
    ],
    ['Unattached', pub({ Unattached: { object: 3 } }), { object: 3, to: null }],
    ['Entered', pub({ Entered: { object: 3, tapped: false, controller: 0 } }), { object: 3, to: null }],
    [
      'ZoneChanged',
      pub({ ZoneChanged: { object: 3, from: 'Battlefield', to: 'Exile', cause: 'Moved' } }),
      { object: 3, to: null },
    ],
    ['a life change', pub({ LifeLost: { player: 0, amount: 1 } }), null],
    ['a bare event', pub('MulliganPhaseBegan'), null],
    ['a hidden event', { Hidden: { CardDrawn: { player: 1 } } }, null],
  ])('reads %s', (_name, event, change) => {
    expect(hostChangeOf(event)).toEqual(change)
  })
})

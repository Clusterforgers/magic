// Goal: the words under the large card in the preview say where the card is, who controls and owns it, and what it is doing in combat, for every zone and every combat role.
import { describe, expect, it } from 'vitest'
import type { CombatView, Zone } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { combatText, controlText, zoneText } from './previewText'

const name = (seat: number): string => ['Ana', 'Bo', 'Cy'][seat] ?? `Seat ${seat + 1}`
const cardName = (id: number): string => ({ 5: 'Grizzly Bears', 6: 'Jace Beleren' })[id] ?? 'a card'

describe('zoneText', () => {
  it.each<[Zone, string]>([
    ['Battlefield', 'On the battlefield'],
    ['Stack', 'On the stack'],
    ['Exile', 'In exile'],
    [{ Hand: 1 }, "In Bo's hand"],
    [{ Library: 2 }, "In Cy's library"],
    [{ Graveyard: 0 }, "In Ana's graveyard"],
    [{ Command: 1 }, "In Bo's command zone"],
  ])('says where a card in %j is', (zone, text) => {
    expect(zoneText(zone, name)).toBe(text)
  })
})

describe('controlText', () => {
  it('names the controller, and the owner only when it is somebody else', () => {
    expect(controlText(card({ owner: 0, controller: 0 }), name)).toBe('Controlled by Ana')
    expect(controlText(card({ owner: 0, controller: 1 }), name)).toBe('Controlled by Bo, owned by Ana')
  })
})

describe('combatText', () => {
  const combat: CombatView = {
    attackers: [
      { object: 1, target: { Player: 1 }, blocked: false, blockers: [] },
      { object: 2, target: { Player: 2 }, blocked: true, blockers: [5, 7] },
      { object: 3, target: { Planeswalker: 6 }, blocked: false, blockers: [] },
    ],
    attackers_declared: true,
    blockers_declared: [1, 2],
  }

  it.each([
    [1, 'Attacking Bo'],
    [2, 'Attacking Cy, blocked'],
    [3, 'Attacking Jace Beleren'],
    [5, 'Blocking a card'],
    [7, 'Blocking a card'],
    [99, null],
  ])('says what object %i is doing', (id, text) => {
    expect(combatText(combat, id, name, cardName)).toBe(text)
  })

  it('names the attacker a blocker is blocking', () => {
    const blocked: CombatView = {
      attackers: [{ object: 5, target: { Player: 0 }, blocked: true, blockers: [7] }],
      attackers_declared: true,
      blockers_declared: [0],
    }
    expect(combatText(blocked, 7, name, cardName)).toBe('Blocking Grizzly Bears')
  })

  it('has nothing to say when nobody attacks', () => {
    const quiet: CombatView = { attackers: [], attackers_declared: false, blockers_declared: [] }
    expect(combatText(quiet, 1, name, cardName)).toBeNull()
  })
})

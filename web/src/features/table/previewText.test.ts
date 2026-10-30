// Goal: the words under the large card in the preview say where the card is, who controls and owns it, what it is doing in combat and, for a host and its attachments, what each attachment gives and what it hangs on, for every zone and every combat role.
import { describe, expect, it } from 'vitest'
import type { CombatView, GivesView, ObjectView, Zone } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { itemsOf } from './attachMarks'
import type { Attach } from './attachMarks'
import { attachLine, combatText, controlText, hostLine, zoneText } from './previewText'

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

describe('attachLine', () => {
  const attach = (name: string, gives: GivesView | undefined, foreign?: number): Attach => ({
    id: 9,
    name,
    foreign,
    items: itemsOf(gives),
  })

  it.each<[string, Attach, string]>([
    [
      'a raise and a keyword',
      attach('Rancor', { power: 2, keywords: ['Trample'] }),
      'Rancor: +2/+0, trample',
    ],
    ['a protection', attach('Sword', { keywords: ['Protection from red'] }), 'Sword: protection from red'],
    [
      'a restriction of another seat',
      attach('Pacifism', { rules: ['CantAttack'] }, 1),
      "Bo's Pacifism: can't attack",
    ],
    ['a set value', attach('Song', { set: [1, 1] }), 'Song: power and toughness set to 1/1'],
    [
      'an umbra',
      attach('Eel Umbra', { rules: ['UmbraArmor'] }),
      'Eel Umbra: umbra armor, so it is not destroyed',
    ],
    ['nothing given', attach('Sword of Fire and Ice', undefined), 'Sword of Fire and Ice'],
  ])('lists %s', (_what, given, line) => {
    expect(attachLine(given, name)).toBe(line)
  })
})

describe('hostLine', () => {
  it.each<[string, Partial<ObjectView>, string | undefined, string | null]>([
    ['an Aura that gives', { gives: { power: 2 } }, 'Hill Giant', 'Attached to Hill Giant, giving +2/+0'],
    ['an Equipment that gives nothing', {}, 'Hill Giant', 'Attached to Hill Giant'],
    ['a Curse on a player', { attached_player: 1 }, 'Bo', 'Enchants Bo'],
    [
      'an Equipment on no host',
      { subtypes: ['Equipment'], types: ['Artifact'] },
      undefined,
      'Not attached to anything',
    ],
    ['a creature', {}, undefined, null],
  ])('says it for %s', (_what, over, host, line) => {
    expect(hostLine(card({ name: 'Rancor', ...over }), host)).toBe(line)
  })
})

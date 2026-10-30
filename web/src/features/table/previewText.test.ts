// Goal: the words under the large card in the preview say where the card is, who controls and owns it, what it is doing in combat and, for a host and its attachments, what each attachment gives and what it hangs on, for every zone and every combat role; the abilities a card activates are worded as it prints them (cost and effect, a keyword with its reminder text, a loyalty ability), without its mana abilities, and the ones the prompt offers are marked.
import { describe, expect, it } from 'vitest'
import type { AbilityRef, ActivatedView, CombatView, GivesView, ObjectView, Zone } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { itemsOf } from './attachMarks'
import type { Attach } from './attachMarks'
import { abilityKey } from './prompt/activations'
import { activatedLines, attachLine, combatText, controlText, hostLine, zoneText } from './previewText'

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

describe('activatedLines', () => {
  const at = (ability: number): AbilityRef => ({ object: 7, ability })
  const draw: ActivatedView = { ability: at(1), cost: '{2}, {T}', effect: 'Draw a card.' }
  const equip: ActivatedView = {
    ability: { object: 7, ability: 0, source: { PrintedKeyword: 3 } },
    cost: '{1}',
    effect: 'Attach to target creature you control.',
    keyword: 'Equip {1}',
  }
  const cycling: ActivatedView = {
    ability: at(2),
    cost: '',
    effect: 'Cycling {2}',
    keyword: 'Cycling {2}',
    unworded: true,
  }
  const tap: ActivatedView = { cost: '{T}', effect: 'Add {G}.', mana: true }
  const loyalty: ActivatedView = { ability: at(3), cost: '+1', effect: 'Scry 2.', loyalty: { Plus: 1 } }
  const bare: ActivatedView = { ability: at(4), cost: '', effect: 'Draw a card.' }
  const blind: ActivatedView = { ability: at(5), cost: '{1}', effect: 'Activated ability 6', unworded: true }

  it.each<[string, ActivatedView[], string[]]>([
    ['a cost and an effect', [draw], ['{2}, {T}: Draw a card.']],
    [
      'a keyword, with its cost and effect as reminder text',
      [equip],
      ['Equip {1} ({1}: Attach to target creature you control.)'],
    ],
    ['a keyword the writer cannot word, by its keyword line', [cycling], ['Cycling {2}']],
    ['a loyalty ability', [loyalty], ['+1: Scry 2.']],
    ['an effect with no cost', [bare], ['Draw a card.']],
    ['an ability named only by its place', [blind], ['Activated ability 6']],
    ['a mana ability, which the mana line tells', [tap], []],
    [
      'mana abilities among others, left out in the order of the card',
      [tap, draw, tap, equip],
      ['{2}, {T}: Draw a card.', 'Equip {1} ({1}: Attach to target creature you control.)'],
    ],
    ['nothing', [], []],
  ])('words %s', (_what, activated, lines) => {
    expect(activatedLines(card({ id: 7, activated }), []).map((line) => line.text)).toEqual(lines)
  })

  it('has no lines for a card the server sent no words for', () => {
    expect(activatedLines(card({ id: 7 }))).toEqual([])
  })

  it('marks the abilities the prompt offers now, by their address', () => {
    const object = card({ id: 7, activated: [draw, equip, cycling] })
    const usable = [abilityKey(at(2)), abilityKey(equip.ability as AbilityRef)]
    expect(activatedLines(object, usable).map((line) => line.now)).toEqual([false, true, true])
    expect(activatedLines(object).map((line) => line.now)).toEqual([false, false, false])
  })

  it('tells two abilities of one index apart by where they come from', () => {
    const granted: ActivatedView = {
      ...draw,
      ability: { object: 7, ability: 1, source: { PrintedKeyword: 4 } },
    }
    const object = card({ id: 7, activated: [draw, granted] })
    expect(
      activatedLines(object, [abilityKey(granted.ability as AbilityRef)]).map((line) => line.now),
    ).toEqual([false, true])
  })

  it('gives each line its own key, even for two of the same words', () => {
    const lines = activatedLines(card({ id: 7, activated: [draw, draw] }))
    expect(new Set(lines.map((line) => line.key)).size).toBe(2)
  })
})

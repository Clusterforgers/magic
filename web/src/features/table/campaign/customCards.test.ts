// Goal: a boss ability or an emblem finds its custom card by name in the content (keeping the table's rules at its level), falls back to a card made from the table's words, says what it is to the fight (a phase to come in no zone yet, a phase listing the boss's phases), and keys a custom card by a stable id below zero.
import { describe, expect, it } from 'vitest'
import type { ObjectView, PlayerView } from '@/protocol/generated'
import { DEV_BOSS_CONTENT } from '../dev/bossContent'
import { devReworkedView } from '../dev/reworkedBoss'
import { reworkedAbilities } from './reworkedBoss'
import { abilityShown, customPreviewId, emblemCard, emblemShown } from './customCards'
import { KIND_WORDS } from './useBossAbilities'

const abilities = (stage: 1 | 2) => reworkedAbilities(devReworkedView(stage, 100))
const PLACE = "In Cy's command zone"

const emblem = (name: string, kind: ObjectView['kind'] = 'Emblem'): ObjectView => ({
  id: 7,
  owner: 1,
  controller: 1,
  zone: { Command: 1 },
  kind,
  name,
  mana_cost: null,
  supertypes: [],
  types: [],
  subtypes: [],
  colors: [],
  power: null,
  toughness: null,
  abilities: [],
  permanent: null,
})

describe('abilityShown', () => {
  it.each([
    [1, 'Brood III', 'Innate', 'Innate · Level III', PLACE, 0],
    [1, 'Deep Pockets', 'Encounter', 'This fight', PLACE, 0],
    [1, 'Whetstone ×2', 'Relic', 'Boss relic', PLACE, 0],
    [1, 'The Nest Stirs', 'Phase', 'Phase 1 at round 8', undefined, 2],
    [2, 'The Nest Stirs', 'Phase', 'Phase 1 · in force', PLACE, 2],
    [2, 'Plague Tide', 'Phase', 'Phase 2 at round 14', undefined, 2],
  ] as const)('stage %i: %s is a %s card, "%s"', (stage, name, kind, kicker, place, phases) => {
    const all = abilities(stage)
    const ability = all.find((each) => each.name === name)
    if (ability === undefined) throw new Error(`no ${name}`)
    const shown = abilityShown(ability, all, DEV_BOSS_CONTENT, PLACE, KIND_WORDS)
    expect(shown.card.kind).toBe(kind)
    expect(shown.card.name).toBe(ability.card)
    expect(shown.card.text).toBe(ability.text)
    expect(shown.kicker).toBe(kicker)
    expect(shown.place).toBe(place)
    expect(shown.phases?.length ?? 0).toBe(phases)
  })

  it.each([
    ['innate', 'Innate'],
    ['modifier', 'Encounter'],
    ['relic', 'Relic'],
    ['next-phase', 'Phase'],
  ] as const)('makes the %s card from the table without content', (kind, cardKind) => {
    const all = abilities(1)
    const ability = all.find((each) => each.kind === kind)
    if (ability === undefined) throw new Error(`no ${kind}`)
    const shown = abilityShown(ability, all, null, PLACE, KIND_WORDS)
    expect(shown.card).toMatchObject({ kind: cardKind, name: ability.card, text: ability.text, art: '' })
  })
})

describe('emblems', () => {
  it.each([
    [emblem('Lantern of Embers'), 'Lantern of Embers'],
    [emblem('Brood III'), 'Brood III'],
    [emblem('Lantern of Embers', 'Card'), undefined],
    [emblem('Monarch'), undefined],
    [undefined, undefined],
  ])('%#: finds the custom card of an emblem by name', (object, name) => {
    expect(emblemCard(object, DEV_BOSS_CONTENT)?.name).toBe(name)
    expect(emblemCard(object, null)).toBeUndefined()
  })

  it("shows a boss's emblem as its ability and a hero's as its card", () => {
    const boss = { seat: 1, boss: devReworkedView(2, 100) } as PlayerView
    const hero = { seat: 0 } as PlayerView
    const phase = emblemCard(emblem('The Nest Stirs'), DEV_BOSS_CONTENT)
    const relic = emblemCard(emblem('Lantern of Embers'), DEV_BOSS_CONTENT)
    if (phase === undefined || relic === undefined) throw new Error('no card')
    expect(emblemShown(phase, boss, DEV_BOSS_CONTENT, 'In their command zone', KIND_WORDS)).toMatchObject({
      kicker: 'Phase 1 · in force',
      place: 'In their command zone',
      phases: [{ card: { name: 'The Nest Stirs' } }, { card: { name: 'Plague Tide' } }],
    })
    expect(emblemShown(relic, hero, DEV_BOSS_CONTENT, 'In your command zone', KIND_WORDS)).toEqual({
      card: relic,
      place: 'In your command zone',
    })
  })
})

describe('customPreviewId', () => {
  it('is stable, below zero and apart for different cards', () => {
    const names = ['Brood III', 'The Nest Stirs', 'Plague Tide', 'Whetstone', '']
    const ids = names.map(customPreviewId)
    expect(ids.every((id) => Number.isInteger(id) && id < 0)).toBe(true)
    expect(new Set(ids).size).toBe(names.length)
    expect(names.map(customPreviewId)).toEqual(ids)
  })
})

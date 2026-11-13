// Goal: a campaign seat says whether it is a Spirit, which boss phase it is in, its scheme in motion and its Omen; every effect the server can name on a player has a short label, a sentence that names the player and a tone, each once per seat; the emblems of a seat's command zone come first and say their rules; and the effects' group name lists the labels.
import { describe, expect, it } from 'vitest'
import type { ObjectView, PlayerEffect, PlayerView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { campaignFacts, effectFacts, effectsName, emblemFacts, seatEffects } from './playerEffects'

const EVERY: PlayerEffect[] = [
  'CantLose',
  'LifeSpared',
  'CantWin',
  'CantCastSpells',
  'CantGainLife',
  'LifeCantChange',
  'NoLifeLossFromDamage',
  { LifeFloor: 1 },
  'CantDraw',
  'SkipsDrawStep',
  'CantSearch',
  'SkipsExtraTurns',
  'NoMaxHandSize',
  { MaxHandSize: 4 },
  { ExtraLandPlays: 2 },
  'HandRevealed',
  'TopRevealed',
  'LooksAtTop',
  { Keyword: 'Hexproof' },
]

const player = (over: Partial<PlayerView>): PlayerView =>
  ({ seat: 1, command: { count: 0, cards: [] }, ...over }) as PlayerView

describe('effectFacts', () => {
  it.each(EVERY.map((effect) => [JSON.stringify(effect), effect] as const))(
    'words %s with a label and a sentence about the player',
    (_, effect) => {
      const facts = effectFacts(effect, 'Bo')
      expect(facts.label).not.toBe('')
      expect(facts.detail).toContain('Bo')
      expect(['help', 'hinder', 'neutral']).toContain(facts.tone)
    },
  )

  it('gives every effect its own key', () => {
    const keys = EVERY.map((effect) => effectFacts(effect, 'Bo').key)
    expect(new Set(keys).size).toBe(keys.length)
  })

  it.each([
    [{ ExtraLandPlays: 1 }, '+1 land'],
    [{ ExtraLandPlays: 2 }, '+2 lands'],
    [{ MaxHandSize: 4 }, 'Hand max 4'],
    [{ Keyword: 'Hexproof' }, 'Hexproof'],
    ['CantLose', "Can't lose"],
  ] as const)('labels %j as %s', (effect, label) => {
    expect(effectFacts(effect, 'Bo').label).toBe(label)
  })
})

describe('emblemFacts and seatEffects', () => {
  const emblem: ObjectView = {
    ...card({ id: 70, owner: 1, controller: 1, zone: { Command: 1 }, permanent: null, name: 'Emblem' }),
    kind: 'Emblem',
    abilities: [],
  }
  const commander = card({ id: 71, owner: 1, controller: 1, zone: { Command: 1 }, permanent: null })
  const objects = { 70: emblem, 71: commander }

  it('lists the emblems of the command zone and not its other cards', () => {
    const seat = player({ command: { count: 2, cards: [71, 70] } })
    const facts = emblemFacts(seat, objects, 'Bo')
    expect(facts.map((fact) => fact.object)).toEqual([70])
    expect(facts[0]?.detail).toBe('Bo has an emblem.')
  })

  it('puts the emblems before the rules and names the group by the labels', () => {
    const seat = player({ command: { count: 1, cards: [70] }, effects: ['CantLose'] })
    const all = seatEffects(seat, objects, 'Bo')
    expect(all.map((fact) => fact.key)).toEqual(['Emblem:70', 'CantLose'])
    expect(effectsName('Bo', all)).toBe("Effects on Bo: Emblem, Can't lose")
  })

  it('has nothing for a seat without effects', () => {
    expect(seatEffects(player({}), objects, 'Bo')).toEqual([])
  })
})

describe('campaignFacts', () => {
  it.each([
    [0, 'Phase 1 of 3', 'next comes Rage'],
    [1, 'Phase 2 of 3', 'next comes Storm'],
    [2, 'Phase 3 of 3', 'last phase'],
  ])('a boss with %i phases begun is in its next phase', (begun, label, detail) => {
    const [fact] = campaignFacts(player({ boss: { phases: ['Rage', 'Storm'], begun } }), 'Grimgut')
    expect(fact?.label).toBe(label)
    expect(fact?.detail).toContain(detail)
  })

  it('a boss with a scheme deck shows the scheme in motion and its Omen, and a seat with schemes but no phases shows no phase', () => {
    const facts = campaignFacts(
      player({ boss: { phases: [], begun: 0, scheme: 'Ambush', omen: 'Feast' } }),
      'Grimgut',
    )
    expect(facts.map((f) => f.label)).toEqual(['Scheme: Ambush', 'Omen: Feast'])
    expect(facts[0]?.detail).toContain('set Ambush in motion')
    expect(facts[1]?.detail).toContain('sets Feast in motion')
  })

  it('a Spirit says so, and an ordinary seat has no campaign facts', () => {
    expect(campaignFacts(player({ spirit: true }), 'Bo').map((f) => f.label)).toEqual(['Spirit'])
    expect(campaignFacts(player({}), 'Bo')).toEqual([])
  })
})

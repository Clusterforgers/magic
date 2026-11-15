// Goal: the card menu's Control group offers summoning sickness removed for a sick permanent and a sacrifice on a permanent the seat controls, control for every seat but the controller of any permanent (the seat itself as "Take control"), onto the battlefield for every seat but the owner of any card in a graveyard or exile that is not an instant, a sorcery or face down out of sight, nothing on other cards, each the exact action and inert for a seat out of the game.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { card, permanent } from '@/test-utils/cards'
import { controlGroup } from './control'
import { OUT_OF_GAME } from './offers'
import type { MenuInput } from './offers'

const NAMES = ['Ana', 'Bo', 'Cy']

function input(object: ObjectView, over: Partial<MenuInput> = {}): MenuInput {
  return {
    seat: 0,
    object,
    players: [0, 1, 2],
    hasCommandZone: false,
    ownerName: 'Ana',
    nameOf: (seat) => NAMES[seat] ?? 'Seat',
    ...over,
  }
}

const labels = (object: ObjectView, over: Partial<MenuInput> = {}): string[] =>
  controlGroup(input(object, over))?.items.map((item) => item.label) ?? []

describe('the Control group of the card menu', () => {
  it.each<[string, ObjectView, string[]]>([
    ['a creature of the seat', card(), ['Sacrifice', 'Give control to Bo', 'Give control to Cy']],
    [
      'a sick creature of the seat',
      permanent({ summoning_sick: true }),
      ['Remove summoning sickness', 'Sacrifice', 'Give control to Bo', 'Give control to Cy'],
    ],
    [
      'a card in the seat graveyard',
      card({ zone: { Graveyard: 0 }, permanent: null }),
      ['Onto the battlefield for Bo', 'Onto the battlefield for Cy'],
    ],
    [
      'a card the seat owns in exile',
      card({ zone: 'Exile', permanent: null }),
      ['Onto the battlefield for Bo', 'Onto the battlefield for Cy'],
    ],
    [
      'an instant in the graveyard',
      card({ zone: { Graveyard: 0 }, permanent: null, types: ['Instant'] }),
      [],
    ],
    ['a card in hand', card({ zone: { Hand: 0 }, permanent: null }), []],
    ['a creature of an opponent', card({ owner: 1, controller: 1 }), ['Take control', 'Give control to Cy']],
    [
      'a card in the graveyard of an opponent',
      card({ zone: { Graveyard: 1 }, permanent: null, owner: 1, controller: 1 }),
      ['Onto the battlefield for you', 'Onto the battlefield for Cy'],
    ],
    [
      'a card in exile that is face down out of sight',
      card({ zone: 'Exile', permanent: null, owner: 1, controller: 1, name: '', face_down: true }),
      [],
    ],
    ['a card in the hand of an opponent', card({ zone: { Hand: 1 }, permanent: null, owner: 1 }), []],
  ])('%s', (_name, object, expected) => {
    expect(labels(object)).toEqual(expected)
  })

  it('sends the exact actions', () => {
    const items = controlGroup(input(permanent({ summoning_sick: true }, { id: 7 })))?.items ?? []
    expect(items.map((item) => item.action)).toEqual([
      { RemoveSickness: { object: 7 } },
      { Sacrifice: { object: 7 } },
      { GiveControl: { object: 7, to: 1 } },
      { GiveControl: { object: 7, to: 2 } },
    ])
  })

  it('sends control of a permanent of an opponent to the seat itself first, then to the others', () => {
    const items = controlGroup(input(card({ id: 9, owner: 1, controller: 1 })))?.items ?? []
    expect(items.map((item) => item.action)).toEqual([
      { GiveControl: { object: 9, to: 0 } },
      { GiveControl: { object: 9, to: 2 } },
    ])
  })

  it('is inert for a seat out of the game', () => {
    const items = controlGroup(input(card(), { off: OUT_OF_GAME }))?.items ?? []
    expect(items.length).toBeGreaterThan(0)
    expect(items.every((item) => item.unavailable === OUT_OF_GAME)).toBe(true)
  })
})

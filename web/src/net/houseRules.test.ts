// Goal: the house rules of a game browser row and of a game lookup are read rule by rule, every rule that is there must have the right kind of value, unknown rules are dropped, and the rules reach the lookup and the listing.
import { describe, expect, it } from 'vitest'
import { readHouseRules } from './houseRules'
import { readGameInfo, readLobbyGame } from './parseLobbies'

describe('reading house rules', () => {
  const good: [string, unknown, unknown][] = [
    ['none', undefined, undefined],
    ['an empty object', {}, {}],
    ['counts', { starting_life: 30, mulligan_cap: 0 }, { starting_life: 30, mulligan_cap: 0 }],
    ['switches', { mana_burn: true, open_hands: false }, { mana_burn: true, open_hands: false }],
    [
      'words',
      { mulligan_style: 'paris', attack_range: 'left' },
      { mulligan_style: 'paris', attack_range: 'left' },
    ],
    ['a hand limit', { max_hand_size: { cards: 9 } }, { max_hand_size: { cards: 9 } }],
    ['no hand limit', { max_hand_size: 'unlimited' }, { max_hand_size: 'unlimited' }],
    ['no commander damage', { commander_damage: 'off' }, { commander_damage: 'off' }],
    [
      'deck rules',
      { deck: { singleton: false, deck_size: 60 } },
      { deck: { singleton: false, deck_size: 60 } },
    ],
    [
      'handicaps',
      { handicaps: [{ seat: 1, starting_life: 10 }] },
      { handicaps: [{ seat: 1, starting_life: 10 }] },
    ],
    ['an unknown rule, dropped', { starting_life: 30, future_rule: 1 }, { starting_life: 30 }],
  ]
  it.each(good)('reads %s', (_name, body, rules) => {
    expect(readHouseRules(body)).toEqual(rules)
  })

  const bad: [string, unknown][] = [
    ['a list', []],
    ['text for a count', { starting_life: 'thirty' }],
    ['a negative count', { starting_life: -1 }],
    ['a number for a switch', { mana_burn: 1 }],
    ['a style nobody knows', { mulligan_style: 'seattle' }],
    ['a hand limit of the wrong shape', { max_hand_size: { cards: 'nine' } }],
    ['commander damage of the wrong word', { commander_damage: 'none' }],
    ['deck rules that are not an object', { deck: true }],
    ['a deck size that is text', { deck: { deck_size: '60' } }],
    ['a handicap without a seat', { handicaps: [{ starting_life: 10 }] }],
    ['handicaps that are not a list', { handicaps: {} }],
  ]
  it.each(bad)('refuses %s', (_name, body) => {
    expect(readHouseRules(body)).toBeNull()
  })
})

describe('the rules in a lookup and a listing', () => {
  const lookup = {
    format: 'commander',
    started: false,
    seats: [{ seat: 0, name: 'Ann' }],
    connected: [0],
    ready: [],
    result: null,
    visibility: 'invite',
    manual_play: false,
  }
  const row = {
    game: 'g1',
    host: 'Ann',
    format: 'commander',
    manual_play: false,
    filled: 1,
    total: 4,
    created_at: '2026-10-10T09:00:00Z',
    age_secs: 3,
  }

  it('reads the rules of a game lookup, and none when the server sends none', () => {
    expect(readGameInfo({ ...lookup, house_rules: { starting_life: 30 } })?.house_rules).toEqual({
      starting_life: 30,
    })
    expect(readGameInfo(lookup)).not.toHaveProperty('house_rules')
  })

  it('reads the rules of a listed game, and none when the server sends none', () => {
    expect(readLobbyGame({ ...row, house_rules: { open_hands: true } })?.house_rules).toEqual({
      open_hands: true,
    })
    expect(readLobbyGame(row)).not.toHaveProperty('house_rules')
  })

  it('refuses a lookup and a row whose rules are wrong', () => {
    expect(readGameInfo({ ...lookup, house_rules: { starting_life: 'x' } })).toBeNull()
    expect(readLobbyGame({ ...row, house_rules: [] })).toBeNull()
  })
})

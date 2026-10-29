// Goal: the table's top bar names the house rules that differ from the game's format as a count that opens the list, and shows nothing for a game that plays by its format's rules.
import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { describeRules } from '@/lib/houseRules'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { HouseRules } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { fixtureGame } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { parseDevSearch } from './devSearch'
import { TableRules } from './TableRules'

function show(format: string, rules: HouseRules | undefined) {
  const fixture = buildFixture({ seats: 2, permanents: 0 })
  const state = (first: PlayState): PlayState => {
    const view = clone(must(first.view))
    view.format = format
    if (rules === undefined) delete view.house_rules
    else view.house_rules = rules
    return { ...first, view, objectsById: indexById(view.objects) }
  }
  return renderTable({ fixture, ui: <TableRules />, state })
}

describe('the house rules in the top bar', () => {
  it.each<[string, string, HouseRules | undefined]>([
    ['no rules', 'Commander', undefined],
    ['rules equal to the format’s own', 'Commander', { starting_life: 40, commander_tax: 2 }],
    ['a Commander rule in a Normal game', 'Normal', { commander_tax: 0 }],
  ])('shows nothing for %s', (_what, format, rules) => {
    show(format, rules)
    expect(screen.queryByRole('button', { name: /House rules/ })).toBeNull()
  })

  it('counts the rules that differ and lists them on press', async () => {
    show('Commander', { starting_life: 30, commander_tax: 0, free_mulligans: 1, open_hands: true })
    const button = screen.getByRole('button', { name: 'House rules: 3 differ from the format' })
    await userEvent.setup().click(button)
    const list = await screen.findByRole('list', { name: 'House rules of this game' })
    expect([...list.children].map((item) => item.textContent)).toEqual([
      'Starting life 30',
      'Open hands',
      'No commander tax',
    ])
  })
})

describe('/dev/table?rules=', () => {
  it.each([
    [1, true],
    ['1', true],
    [true, true],
    [0, undefined],
    ['yes', undefined],
  ])('reads %j', (asked, rules) => {
    expect(parseDevSearch({ rules: asked }).rules).toBe(rules)
  })

  it('gives the fixture table rules that differ from Commander\x27s own, and none by default', () => {
    const state = (rules?: true) => fixtureGame({ seats: 2, rules }).session.store.getState()
    expect(describeRules(state(true).view?.house_rules, 'Commander')).toEqual([
      'Starting life 30',
      'Open hands',
      'No commander tax',
    ])
    expect(state().view?.house_rules).toBeUndefined()
  })
})

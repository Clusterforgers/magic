// Goal: the boss's abilities bar stays one compact row (each chip its icon and name, the phases first, the rules on press), and the boss's pod under it leaves out what the bar shows (its phase and emblems) so it keeps room for its life, zones and corner; without the bar (Grid) the pod keeps its phase chip.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from '../GameTable'
import { withBoss } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { withoutBossBar } from '../playerEffects'
import type { EffectFacts } from '../playerEffects'
import { barOrder } from './bossAbilities'
import type { BossAbility, BossAbilityKind } from './bossAbilities'

const ability = (kind: BossAbilityKind, name: string): BossAbility => ({
  key: name,
  kind,
  name,
  text: '',
  summary: '',
})

function fight(mode?: 'grid') {
  return renderTable({
    fixture: withBoss(buildFixture({ seats: 4 }), 0),
    options: mode === undefined ? {} : { mode },
    ui: <GameTable />,
  })
}

const bossPod = (): HTMLElement => screen.getByRole('region', { name: /, boss$/ })

describe('barOrder', () => {
  it.each([
    [
      ['relic', 'next-phase', 'innate', 'modifier', 'phase'],
      ['phase', 'next-phase', 'innate', 'modifier', 'relic'],
    ],
    [
      ['relic', 'relic', 'phase'],
      ['phase', 'relic', 'relic'],
    ],
    [[], []],
  ] as const)('orders %j as %j, each kind keeping its order', (kinds, expected) => {
    const abilities = kinds.map((kind, at) => ability(kind, `${kind}-${at}`))
    const ordered = barOrder(abilities)
    expect(ordered.map((one) => one.kind)).toEqual(expected)
    const relics = ordered.filter((one) => one.kind === 'relic').map((one) => one.name)
    expect(relics).toEqual([...relics].sort())
  })
})

describe('withoutBossBar', () => {
  const fact = (key: string): EffectFacts => ({ key, icon: 'boss', label: key, tone: 'neutral', detail: key })
  it.each([
    ['Boss', false],
    ['Emblem:7', false],
    ['Eyes', true],
    ['Hexproof', true],
  ])('%s stays: %s', (key, stays) => {
    expect(withoutBossBar([fact(key)]).length === 1).toBe(stays)
  })
})

describe('the abilities bar at the table', () => {
  it('is one row of chips with names only', () => {
    fight()
    const bar = screen.getByRole('list', { name: /abilities$/ })
    expect(bar).toHaveClass('h-7', 'overflow-hidden')
    const chips = within(bar).getAllByRole('button')
    expect(chips[0]).toHaveAttribute('data-kind', 'innate')
    for (const chip of chips) {
      const visible = [...chip.querySelectorAll('span')].filter((span) => !span.classList.contains('sr-only'))
      expect(visible.map((span) => span.textContent)).toHaveLength(1)
    }
  })

  it('leaves the phase and the emblems off the boss pod, and draws no emblem as a card', () => {
    const { fixture } = fight()
    const pod = bossPod()
    expect(pod.querySelector('[data-effect="Boss"]')).toBeNull()
    expect(pod.querySelector('[data-effect^="Emblem:"]')).toBeNull()
    for (const emblem of fixture.view.objects.filter((object) => object.kind === 'Emblem')) {
      expect(pod.querySelector(`[data-object-id="${emblem.id}"]`)).toBeNull()
    }
  })

  it('without the bar (Grid) keeps the phase chip on the pod', () => {
    fight('grid')
    expect(screen.queryByRole('list', { name: /abilities$/ })).toBeNull()
    expect(bossPod().querySelector('[data-effect="Boss"]')).not.toBeNull()
  })
})

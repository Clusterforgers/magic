// Goal: the boss's abilities bar stays one compact row (each chip its icon and name, the phase in force and the scheme first, the rules on press and every ability in full behind "All"), carries the phase track, and the boss's pod under it leaves out what the bar shows (phase, scheme, Omen, emblems) so it keeps room for its life, zones and corner; without the bar (Grid) the pod keeps them.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from '../GameTable'
import { DEV_BOSS_CONTENT, DEV_BOSS_EMBLEMS } from '../dev/bossContent'
import { withBoss } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { withoutBossBar } from '../playerEffects'
import type { EffectFacts } from '../playerEffects'
import { barOrder } from './bossAbilities'
import type { BossAbility, BossAbilityKind } from './bossAbilities'
import { DelveContentContext } from './useBossAbilities'

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
    ui: (
      <DelveContentContext value={DEV_BOSS_CONTENT}>
        <GameTable />
      </DelveContentContext>
    ),
  })
}

const bossPod = (): HTMLElement => screen.getByRole('region', { name: /, boss$/ })

describe('barOrder', () => {
  it.each([
    [
      ['passive', 'next-phase', 'scheme', 'omen', 'phase'],
      ['phase', 'scheme', 'omen', 'next-phase', 'passive'],
    ],
    [
      ['passive', 'passive', 'phase'],
      ['phase', 'passive', 'passive'],
    ],
    [[], []],
  ] as const)('orders %j as %j, each kind keeping its order', (kinds, expected) => {
    const abilities = kinds.map((kind, at) => ability(kind, `${kind}-${at}`))
    const ordered = barOrder(abilities)
    expect(ordered.map((one) => one.kind)).toEqual(expected)
    const passives = ordered.filter((one) => one.kind === 'passive').map((one) => one.name)
    expect(passives).toEqual([...passives].sort())
  })
})

describe('withoutBossBar', () => {
  const fact = (key: string): EffectFacts => ({ key, icon: 'boss', label: key, tone: 'neutral', detail: key })
  it.each([
    ['Boss', false],
    ['Scheme', false],
    ['Omen', false],
    ['Emblem:7', false],
    ['Eyes', true],
    ['Hexproof', true],
  ])('%s stays: %s', (key, stays) => {
    expect(withoutBossBar([fact(key)]).length === 1).toBe(stays)
  })
})

describe('the abilities bar at the table', () => {
  it('is one row of chips with names only, the phase in force first, the summary for a screen reader', () => {
    fight()
    const bar = screen.getByRole('list', { name: /abilities$/ })
    expect(bar).toHaveClass('h-7', 'overflow-hidden')
    const chips = within(bar).getAllByRole('button')
    expect(chips[0]).toHaveAttribute('data-kind', 'phase')
    expect(chips[1]).toHaveAttribute('data-kind', 'scheme')
    for (const chip of chips) {
      const visible = [...chip.querySelectorAll('span')].filter((span) => !span.classList.contains('sr-only'))
      expect(visible.map((span) => span.textContent)).toHaveLength(1)
    }
    expect(within(bar).getByText('Escalation')).toBeInTheDocument()
  })

  it('lists every ability in full behind "All"', async () => {
    const { user } = fight()
    await user.click(screen.getByRole('button', { name: /^All \d+ abilities of / }))
    const full = await screen.findAllByText((_, element) => element?.getAttribute('data-slot') === 'boss-ability-full')
    expect(full.length).toBeGreaterThanOrEqual(DEV_BOSS_EMBLEMS.length)
    expect(screen.getByText(DEV_BOSS_CONTENT.cards.find((one) => one.name === 'Warpath')?.text ?? '-')).toBeInTheDocument()
  })

  it('carries the phase track, and the boss pod leaves out what the bar shows', () => {
    const { container } = fight()
    expect(container.querySelector('[data-slot="boss-abilities"] [data-slot="boss-track"]')).not.toBeNull()
    const pod = bossPod()
    expect(pod.querySelector('[data-slot="boss-track"]')).toBeNull()
    for (const key of ['Boss', 'Scheme', 'Omen']) expect(pod.querySelector(`[data-effect="${key}"]`)).toBeNull()
    expect(pod.querySelector('[data-effect^="Emblem:"]')).toBeNull()
  })

  it('draws no emblem as a card of the command zone', () => {
    const { fixture } = fight()
    const emblems = fixture.view.objects.filter((object) => object.kind === 'Emblem')
    expect(emblems).toHaveLength(DEV_BOSS_EMBLEMS.length)
    const pod = bossPod()
    expect(pod.querySelector('[data-slot="corner-commander"] [data-object-id]')).not.toBeNull()
    for (const emblem of emblems) {
      expect(pod.querySelector(`[data-object-id="${emblem.id}"]`)).toBeNull()
    }
  })

  it('without the bar (Grid) keeps the track and the chips on the pod', () => {
    fight('grid')
    expect(screen.queryByRole('list', { name: /abilities$/ })).toBeNull()
    const pod = bossPod()
    expect(pod.querySelector('[data-slot="boss-track"]')).not.toBeNull()
    expect(pod.querySelector('[data-effect="Scheme"]')).not.toBeNull()
  })
})

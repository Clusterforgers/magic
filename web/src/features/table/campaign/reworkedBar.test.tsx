// Goal: on a desktop and on a phone a reworked boss's phase pill sits in the top bar ("Phase 1 in 3 rounds", short "P1 in 3"; then the phase's name with Phase 2's countdown) at one width whatever it says, never in the abilities row; the row keeps the innate and modifier as chips and every part behind "All"; the pill, each chip, each row of "All", the boss's emblems and an ally's relic (Grid) open their custom card in the card preview (its frame, what it is to the fight, a phase's steps that move between the phases); the boss's Satchel list marks reusable cards "returns · +{N}" and the once card "once"; in Grid the pod's track names the countdown.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { DEV_BOSS_CONTENT } from '../dev/bossContent'
import { withBoss } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { DEV_HERO_RELIC } from '../dev/reworkedBoss'
import type { DevRework } from '../dev/reworkedBoss'

const PHONE = { width: 390, height: 844 }
const DESKTOP = { width: 1400, height: 900 }

function fight(stage: DevRework, viewport: { width: number; height: number }, mode?: 'grid', content = true) {
  const fixture = withBoss(buildFixture({ seats: 4 }), 0, stage)
  return renderTable({
    fixture,
    ...(content ? { content: DEV_BOSS_CONTENT } : {}),
    options: { viewport, ...(mode === undefined ? {} : { mode }) },
  })
}

const pill = (): HTMLElement =>
  must(document.querySelector<HTMLElement>('[data-region="bar"] [data-slot="phase-countdown"]'))

const preview = (): HTMLElement => screen.getByTestId('card-preview')

describe.each([
  ['desktop', DESKTOP],
  ['phone', PHONE],
] as const)('a reworked boss on a %s', (_, viewport) => {
  it('counts down to Phase 1 in the top bar and keeps the innate and modifier in the row', () => {
    fight(1, viewport)
    expect(pill()).toHaveAccessibleName('Phase 1 in 3 rounds.')
    expect(pill()).toHaveTextContent('Phase 1 in 3 rounds')
    expect(must(pill().querySelector('[data-slot="phase-countdown-short"]'))).toHaveTextContent('P1 in 3')
    expect(pill()).not.toHaveAttribute('data-begun')
    expect(document.querySelector('[data-slot="boss-abilities"] [data-slot="phase-countdown"]')).toBeNull()
    const row = screen.getByRole('list', { name: /abilities$/ })
    const kinds = within(row)
      .getAllByRole('button', { hidden: true })
      .map((chip) => chip.getAttribute('data-kind'))
    expect(kinds.slice(0, 2)).toEqual(['innate', 'modifier'])
    expect(kinds).not.toContain('next-phase')
    expect(kinds).not.toContain('phase')
    expect(within(row).getByText('Brood III')).toBeInTheDocument()
    expect(within(row).getByText('Deep Pockets')).toBeInTheDocument()
  })

  it('names the phase in force once it began, the pill keeping its width', () => {
    fight(1, viewport)
    const before = pill().className
    fight(2, viewport)
    const pills = document.querySelectorAll<HTMLElement>('[data-slot="phase-countdown"]')
    const after = must(pills[pills.length - 1])
    expect(after).toHaveAttribute('data-begun', 'true')
    expect(after).toHaveTextContent('The Nest Stirs')
    expect(must(after.querySelector('[data-slot="phase-countdown-short"]'))).toHaveTextContent('P2 in 5')
    expect(after).toHaveAccessibleName(/Phase 1: The Nest Stirs\. Phase 2 in 5 rounds\./)
    const width = (classes: string) => classes.split(' ').filter((name) => /^(\w+:)?w-/.test(name))
    expect(width(after.className)).toEqual(width(before))
  })

  it("opens the phase it names in the card preview, whose steps move to the boss's other phase", async () => {
    const { user } = fight(1, viewport)
    await user.click(pill())
    expect(preview()).toHaveAttribute('data-custom', 'Phase')
    expect(preview()).toHaveAttribute('data-pinned', 'true')
    expect(within(preview()).getByRole('figure', { name: 'The Nest Stirs' })).toBeInTheDocument()
    expect(
      within(preview()).getByText('Phase 1 at round 8', { selector: '[data-slot="preview-kicker"]' }),
    ).toBeInTheDocument()
    const steps = within(preview()).getAllByRole('button', { name: /at round/ })
    expect(steps.map((step) => step.textContent)).toEqual([
      'Phase 1 at round 8The Nest Stirs',
      'Phase 2 at round 14Plague Tide',
    ])
    expect(steps[0]).toHaveAttribute('aria-current', 'true')
    await user.click(must(steps[1]))
    expect(within(preview()).getByRole('figure', { name: 'Plague Tide' })).toBeInTheDocument()
    await user.click(within(preview()).getByRole('button', { name: 'Close preview' }))
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })

  it('opens a chip and a row of "All" as their cards', async () => {
    const { user } = fight(1, viewport)
    const row = screen.getByRole('list', { name: /abilities$/ })
    await user.click(within(row).getByRole('button', { name: /^Brood III/ }))
    expect(preview()).toHaveAttribute('data-custom', 'Innate')
    expect(within(preview()).getByText('Innate · Level III')).toBeInTheDocument()
    expect(within(preview()).getByText(/^In .+'s command zone\.$/)).toBeInTheDocument()
    await user.keyboard('{Escape}')
    await user.click(screen.getByRole('button', { name: /^All \d+ abilities of / }))
    expect(screen.getAllByText(/Innate · Level III/).length).toBeGreaterThan(0)
    const rows = await screen.findAllByRole('button', { name: /Whetstone ×2/ })
    await user.click(must(rows.find((row) => row.closest('[data-slot="boss-ability-full"]') !== null)))
    expect(preview()).toHaveAttribute('data-custom', 'Relic')
    expect(within(preview()).getByRole('figure', { name: 'Whetstone' })).toBeInTheDocument()
    expect(within(preview()).getByText('Boss relic')).toBeInTheDocument()
  })
})

describe('custom cards as emblems', () => {
  it.each([
    ['desktop', DESKTOP],
    ['phone', PHONE],
  ] as const)("opens a hero's relic from an ally's pod (Grid) on a %s, in its frame with its rarity", async (_, viewport) => {
    const { user } = fight(2, viewport, 'grid')
    const chips = [...document.querySelectorAll<HTMLElement>('[data-slot="player-effect"]')]
    const relic = must(chips.find((chip) => chip.textContent === DEV_HERO_RELIC))
    await user.click(relic)
    expect(preview()).toHaveAttribute('data-custom', 'Relic')
    const frame = within(preview()).getByRole('figure', { name: DEV_HERO_RELIC })
    expect(frame).toHaveAttribute('data-rarity', 'Rare')
    expect(within(frame).getByText('Removal')).toBeInTheDocument()
    expect(within(preview()).getByText(/^In .+'s command zone\.$/)).toBeInTheDocument()
    await user.click(relic)
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })

  it("draws a boss's phase emblem on its pod (Grid) as that phase, with the boss's phases", async () => {
    const { user, store } = fight(2, DESKTOP, 'grid')
    const objects = Object.values(store.getState().objectsById)
    const phase = must(objects.find((object) => object.name === 'The Nest Stirs'))
    await user.click(must(document.querySelector<HTMLElement>(`[data-object-id="${phase.id}"]`)))
    expect(preview()).toHaveAttribute('data-custom', 'Phase')
    expect(
      within(preview()).getByText('Phase 1 · in force', { selector: '[data-slot="preview-kicker"]' }),
    ).toBeInTheDocument()
    expect(within(preview()).getAllByRole('button', { name: /^Phase \d/ })).toHaveLength(2)
  })

  it('draws the custom card from the words of the table while the content is not known', async () => {
    const { user } = fight(1, DESKTOP, undefined, false)
    const row = screen.getByRole('list', { name: /abilities$/ })
    await user.click(within(row).getByRole('button', { name: /^Deep Pockets/ }))
    expect(preview()).toHaveAttribute('data-custom', 'Encounter')
    expect(within(preview()).getByRole('figure', { name: 'Deep Pockets' })).toHaveTextContent(
      "At the beginning of each player's draw step",
    )
  })
})

describe("the reworked boss's Satchel", () => {
  it.each([
    ['desktop', DESKTOP],
    ['phone', PHONE],
  ] as const)('marks its cards on a %s', async (_, viewport) => {
    const { user } = fight(2, viewport)
    if (viewport === PHONE) {
      const tile = document.querySelector<HTMLElement>('[data-tile-seat="3"]')
      if (tile !== null) await user.click(tile)
    }
    await user.click(must(document.querySelector<HTMLElement>('button[data-zone="satchel"]')))
    const marks = await screen.findAllByText(
      (_, element) => element?.getAttribute('data-slot') === 'satchel-mark',
    )
    expect(marks.map((mark) => mark.textContent?.split('.')[0])).toEqual([
      'Rat Outreturns · +{4}',
      'Feed the Swarmreturns · +{0}',
      'Village Ritesreturns · +{0}',
      'Second Harvestonce',
    ])
  })
})

describe('without the bar (Grid)', () => {
  it("names the countdown on the boss's pod", () => {
    fight(1, DESKTOP, 'grid')
    const pod = screen.getByRole('region', { name: /, boss$/ })
    expect(pod.querySelector('[data-slot="boss-track-countdown"]')).toHaveTextContent('Phase 1 in 3 rounds')
  })
})

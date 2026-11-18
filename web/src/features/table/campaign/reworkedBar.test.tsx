// Goal: on a desktop and on a phone a reworked boss's bar shows its countdown pill ("Phase 1 in 3 rounds", then the phase's name with Phase 2's countdown) at one width whatever it says, the innate with its level and the modifier as chips, no phase to come as a chip (the pill's popover lists both with their rounds), and every part behind "All"; the boss's Satchel list marks reusable cards "returns · +{N}" and the once card "once"; in Grid the pod's track names the countdown.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { withBoss } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { withReworkedBoss } from '../dev/reworkedBoss'
import type { DevRework } from '../dev/reworkedBoss'

const PHONE = { width: 390, height: 844 }
const DESKTOP = { width: 1400, height: 900 }

function fight(stage: DevRework, viewport: { width: number; height: number }, mode?: 'grid') {
  const fixture = withReworkedBoss(withBoss(buildFixture({ seats: 4 }), 0), stage)
  return renderTable({ fixture, options: { viewport, ...(mode === undefined ? {} : { mode }) } })
}

const pill = (): HTMLElement =>
  must(document.querySelector<HTMLElement>('[data-slot="boss-abilities"] [data-slot="phase-countdown"]'))

describe.each([
  ['desktop', DESKTOP],
  ['phone', PHONE],
] as const)('a reworked boss on a %s', (_, viewport) => {
  it('counts down to Phase 1 and keeps the innate and modifier in the row', () => {
    fight(1, viewport)
    expect(pill()).toHaveTextContent('Phase 1 in 3 rounds')
    expect(pill()).not.toHaveAttribute('data-begun')
    const row = screen.getByRole('list', { name: /abilities$/ })
    const kinds = within(row)
      .getAllByRole('button', { hidden: true })
      .map((chip) => chip.getAttribute('data-kind'))
    expect(kinds.slice(0, 2)).toEqual(['innate', 'modifier'])
    expect(kinds).not.toContain('next-phase')
    expect(kinds).not.toContain('phase')
    expect(within(row).getByText('Brood III')).toBeInTheDocument()
    expect(within(row).getByText('Deep Pockets')).toBeInTheDocument()
    expect(document.querySelector('[data-slot="boss-abilities"] [data-slot="boss-bar-track"]')).toBeNull()
  })

  it('names the phase in force once it began, the pill keeping its width', () => {
    fight(1, viewport)
    const before = pill().className
    fight(2, viewport)
    const pills = document.querySelectorAll<HTMLElement>('[data-slot="phase-countdown"]')
    const after = must(pills[pills.length - 1])
    expect(after).toHaveAttribute('data-begun', 'true')
    expect(after).toHaveTextContent('The Nest Stirs')
    expect(after).toHaveAccessibleName(/Phase 1: The Nest Stirs\. Phase 2 in 5 rounds\./)
    const width = (classes: string) => classes.split(' ').filter((name) => /^(sm:)?w-/.test(name))
    expect(width(after.className)).toEqual(width(before))
  })

  it('lists both phases with their rounds behind the pill and every part behind "All"', async () => {
    const { user } = fight(1, viewport)
    await user.click(pill())
    const steps = await screen.findAllByText(
      (_, element) => element?.getAttribute('data-slot') === 'phase-countdown-step',
    )
    expect(steps.map((step) => step.textContent)).toEqual([
      expect.stringContaining('Phase 1 at round 8'),
      expect.stringContaining('Phase 2 at round 14'),
    ])
    await user.keyboard('{Escape}')
    await user.click(screen.getByRole('button', { name: /^All \d+ abilities of / }))
    expect((await screen.findAllByText('Whetstone ×2')).length).toBeGreaterThan(0)
    expect(screen.getAllByText(/Innate · Level III/).length).toBeGreaterThan(0)
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

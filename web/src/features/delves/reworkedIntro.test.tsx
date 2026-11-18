// Goal: the intro of a reworked boss's fight shows its two effects (the innate with its level, the modifier), its two phases with "Phase 1 at round 8", its Satchel with each card's mark and its relics with copies counted, at a desktop and a phone width with nothing wider than one column on a phone.
import { render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { BossIntro } from './BossIntro'
import { DEV_CONTENT, devView } from './dev/devDelve'

const WIDTHS = [1400, 390] as const
const initial = window.innerWidth

afterEach(() => {
  window.innerWidth = initial
})

const section = (name: RegExp): HTMLElement => screen.getByRole('region', { name })

describe.each(WIDTHS)('the intro of a reworked boss at %i px', (width) => {
  it('shows the effects, the phases by round, the Satchel and the relics', () => {
    window.innerWidth = width
    render(<BossIntro view={devView('rework')} content={DEV_CONTENT} />)
    expect(screen.getByText('· Level III')).toBeInTheDocument()
    const effects = section(/^What Skrix, the Rat Queen does$/)
    expect(within(effects).getByText('Brood III')).toBeInTheDocument()
    expect(within(effects).getByText(/Innate · Level III/)).toBeInTheDocument()
    expect(within(effects).getByText('Deep Pockets')).toBeInTheDocument()
    const phases = section(/^Phases$/)
    expect(within(phases).getByText(/Phase 1 at round 8/)).toBeInTheDocument()
    expect(within(phases).getByText(/Phase 2 at round 14/)).toBeInTheDocument()
    const satchel = section(/^Satchel$/)
    const marks = within(satchel).getAllByRole('listitem')
    expect(marks.map((mark) => mark.textContent?.split('.')[0])).toEqual([
      'Rat Outreturns · +{0}',
      'Feed the Swarmreturns · +{0}',
      'Village Ritesreturns · +{0}',
      'Second Harvestonce',
    ])
    expect(within(section(/^Relics$/)).getByText('Whetstone ×2')).toBeInTheDocument()
    // Each list is one column until the small breakpoint, so a phone never scrolls sideways.
    for (const list of [effects, phases, satchel].map((part) => must(part.querySelector('ul'))))
      expect([...list.classList].filter((name) => /^grid-cols-/.test(name))).toEqual([])
  })
})

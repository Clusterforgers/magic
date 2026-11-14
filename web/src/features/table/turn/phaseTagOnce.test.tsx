// Goal: the phase is named once in the top bar: the badge's tag keeps its icon and leaves the name to the step ladder from 1024 px, shows it between phone and that width, and keeps the icon alone on a phone.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { phaseOf } from './phase'
import { PhaseTag } from './PhaseTag'

describe('the phase tag', () => {
  it('names the phase only where the ladder does not', () => {
    const phase = phaseOf('Main1')
    const { container } = render(<PhaseTag phase={phase} yours />)
    const name = container.querySelector('[data-slot="phase-name"]')
    expect(name).toHaveTextContent(phase.name)
    expect(name).toHaveClass('max-sm:hidden', 'lg:hidden')
    expect(container.querySelector('svg')).not.toBeNull()
  })
})

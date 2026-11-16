// Goal: the newest stack item is staged as the focus (raised, ringed, rising into view after its card lands where motion is welcome, still under reduced motion), the items under it step back to 0.8 and no further, and a hover or focus brings one back whole.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { blockAfter, rulesIn } from '@/test-utils/cssBlocks'
import { renderTable } from '@/test-utils/tableHarness'
import { StackChips } from './StackChips'

const css = readFileSync(join(import.meta.dirname, 'fx/fx.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
const motion = blockAfter(css, '@media (prefers-reduced-motion: no-preference)')
const reduced = blockAfter(css, '@media (prefers-reduced-motion: reduce)')

const chipButtons = () =>
  within(screen.getByRole('list', { name: 'Spells on the stack, top first' }))
    .getAllByRole('button')
    .filter((button) => button.hasAttribute('data-stack-chip'))

describe('the staged stack', () => {
  it('stages the top chip alone and steps the rest back to 0.8, whole again on hover or focus', () => {
    renderTable({ seats: 4, ui: <StackChips /> })
    const [top, ...rest] = chipButtons()
    expect(top).toHaveClass('fx-stage-top', 'border-primary')
    expect(top).not.toHaveClass('opacity-80')
    expect(rest.length).toBeGreaterThan(0)
    for (const chip of rest) {
      expect(chip).toHaveClass('opacity-80', 'hover:opacity-100', 'focus-visible:opacity-100')
      expect(chip).not.toHaveClass('fx-stage-top')
    }
  })

  it('raises the top chip with a shadow in every motion setting', () => {
    const still = rulesIn(css.replace(motion, '').replace(reduced, '')).find(
      (rule) => rule.selector === '.fx-stage-top',
    )
    expect(still?.body).toMatch(/box-shadow/)
  })

  it('lets the top chip rise and settle only where motion is welcome, once the leg to it has landed', () => {
    const rule = rulesIn(motion).find((entry) => entry.selector === '.fx-stage-top')
    expect(rule?.body).toMatch(/animation: fx-arrive calc\(var\(--fx-arrive-ms\) \* var\(--fx-slow, 1\)\)/)
    expect(rule?.body).toMatch(/calc\(var\(--fx-travel-ms\) \* 0\.8\) both/)
    expect(blockAfter(motion, '@keyframes fx-arrive')).toMatch(/scale: 1\.08/)
    expect(reduced).not.toMatch(/fx-stage-top|fx-arrive/)
  })
})

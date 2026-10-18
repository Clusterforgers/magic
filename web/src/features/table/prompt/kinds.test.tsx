/** Goal: every decision kind has a prompt in the registry, and a kind whose prompt is not built yet shows a placeholder that names the decision and offers nothing to press, so the game can never show a blank where a decision is waiting. */
import { within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { DECISION_LABEL } from '../turnText'
import type { DecisionName } from '../turnText'
import { PROMPTS } from './registry'

const NAMES = Object.keys(DECISION_LABEL) as DecisionName[]
// Part 2b to 2e take these off the list one at a time as they replace the placeholder in `kinds/`.
const UNBUILT: readonly DecisionName[] = ['DeclareAttackers', 'DeclareBlockers', 'AssignDamage']

describe('the registry', () => {
  it('has a prompt for each of the eight decision kinds and for nothing else', () => {
    expect(Object.keys(PROMPTS).toSorted()).toEqual([...NAMES].toSorted())
  })
})

describe('a decision kind with no prompt yet', () => {
  it.each(UNBUILT)('shows a placeholder for %s', (name) => {
    const { container } = renderTable({
      state: (state) => ({ ...state, pending: { id: 5, decision: decisionFor(name) } }),
    })
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('data-decision', name)
    expect(line).toHaveTextContent(`Your decision: ${DECISION_LABEL[name]}`)
    const words = within(line).getByText('This decision is not supported in the UI yet')
    expect(words).toHaveAttribute('aria-live', 'polite')
    expect(within(line).queryAllByRole('button')).toEqual([])
  })

  it('is not the prompt of Priority, which is built', () => {
    const { container } = renderTable()
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('data-decision', 'Priority')
    expect(line).not.toHaveTextContent('not supported')
  })
})

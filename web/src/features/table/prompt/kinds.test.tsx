/** Goal: every decision kind has a prompt in the registry: the kinds the table has always asked are built, the decisions of spells and abilities point at the placeholder until their tasks, and the placeholder names the decision and offers nothing to press, so the game can never show a blank where a decision is waiting. */
import { render, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { DECISION_LABEL } from '../turnText'
import type { DecisionName } from '../turnText'
import { PromptContext } from './PromptContext'
import type { PromptApi } from './PromptContext'
import { UnsupportedPrompt } from './PromptLayout'
import { PROMPTS } from './registry'

const NAMES = Object.keys(DECISION_LABEL) as DecisionName[]
// The kinds with a prompt; the others are the effects' decisions, whose prompts come with their tasks.
const BUILT: DecisionName[] = [
  'Mulligan',
  'BottomCards',
  'Priority',
  'PayMana',
  'DeclareAttackers',
  'DeclareBlockers',
  'AssignDamage',
  'Discard',
]
const LATER = NAMES.filter((name) => !BUILT.includes(name))

describe('the registry', () => {
  it('has a prompt for each decision kind and for nothing else', () => {
    expect(Object.keys(PROMPTS).toSorted()).toEqual([...NAMES].toSorted())
    expect(LATER).toHaveLength(11)
  })

  it.each(BUILT)('has a built prompt for %s, not the placeholder', (name) => {
    expect(PROMPTS[name]).not.toBe(UnsupportedPrompt)
  })

  it.each(LATER)('points %s at the placeholder until its task builds the prompt', (name) => {
    expect(PROMPTS[name]).toBe(UnsupportedPrompt)
  })

  it('shows the prompt of Priority, which is built, at the fixture table', () => {
    const { container } = renderTable()
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('data-decision', 'Priority')
    expect(line).not.toHaveTextContent('not supported')
  })
})

describe('the placeholder for a kind with no prompt yet', () => {
  it.each(NAMES)('names the decision %s, says it is not supported and offers nothing to press', (name) => {
    const api: PromptApi = {
      id: 5,
      name,
      decision: decisionFor(name),
      answer: () => false,
      selection: {
        ids: new Set(),
        has: () => false,
        toggle: () => undefined,
        set: () => undefined,
        clear: () => undefined,
      },
    }
    const { container } = render(
      <PromptContext value={api}>
        <div data-slot="decision-line">
          <UnsupportedPrompt />
        </div>
      </PromptContext>,
    )
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveTextContent(`Your decision: ${DECISION_LABEL[name]}`)
    const words = within(line).getByText('This decision is not supported in the UI yet')
    expect(words).toHaveAttribute('aria-live', 'polite')
    expect(within(line).queryAllByRole('button')).toEqual([])
  })
})

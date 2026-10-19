/** Goal: every decision kind has a prompt in the registry and the eight the table has always asked are built; the placeholder for the others names the decision and offers nothing to press, except that the target decision can take the cast back, so the table never shows a blank or sticks where a decision is waiting. */
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { Answer } from '@/protocol/generated'
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
// The kinds the table has always asked; the questions of spells and abilities get their prompts with their tasks.
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

describe('the registry', () => {
  it('has a prompt for each decision kind and for nothing else', () => {
    expect(Object.keys(PROMPTS).toSorted()).toEqual([...NAMES].toSorted())
    expect(NAMES).toHaveLength(19)
  })

  it.each(BUILT)('has a built prompt for %s, not the placeholder', (name) => {
    expect(PROMPTS[name]).not.toBe(UnsupportedPrompt)
  })

  it('shows the prompt of Priority, which is built, at the fixture table', () => {
    const { container } = renderTable()
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('data-decision', 'Priority')
    expect(line).not.toHaveTextContent('not supported')
  })
})

describe('the placeholder for the target decision', () => {
  const faced = (canCancel: boolean) => {
    const answers: Answer[] = []
    const api: PromptApi = {
      id: 5,
      name: 'ChooseTargets',
      decision: decisionFor('ChooseTargets'),
      answer: (answer) => {
        answers.push(answer)
        return true
      },
      selection: {
        ids: new Set(),
        has: () => false,
        toggle: () => undefined,
        set: () => undefined,
        clear: () => undefined,
      },
    }
    const Prompt = PROMPTS.ChooseTargets
    const options = { source: 1, spec: 0, legal: [], min: 1, max: 1, can_cancel: canCancel }
    render(
      <PromptContext value={api}>
        <Prompt options={options} />
      </PromptContext>,
    )
    return answers
  }

  it('takes the cast back with Cancel, since nothing is paid yet, so a spell chosen by mistake never sticks', async () => {
    const answers = faced(true)
    await userEvent.setup().click(screen.getByRole('button', { name: 'Cancel the spell' }))
    expect(answers).toEqual([{ ChooseTargets: 'Cancel' }])
  })

  it('offers nothing to press when the cast cannot be taken back', () => {
    const answers = faced(false)
    expect(screen.queryAllByRole('button')).toEqual([])
    expect(screen.getByText('Choosing targets is not supported in the UI yet')).toBeInTheDocument()
    expect(answers).toEqual([])
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

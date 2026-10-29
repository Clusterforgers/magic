/** Goal: every decision kind has a prompt in the registry and the nineteen the table is built for (the eight it has always asked, the targets of a spell or ability, the order of triggers, yes or no, an option, modes, cards, an order of cards, a number, which replacement applies, a division and the optional costs) are built; the stand-in for the others is the generic prompt (`GenericPrompt.test.tsx`), so the table never shows a blank where a decision is waiting. */
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { DECISION_LABEL } from '../turnText'
import type { DecisionName } from '../turnText'
import { UnsupportedPrompt } from './PromptLayout'
import { PROMPTS } from './registry'

const NAMES = Object.keys(DECISION_LABEL) as DecisionName[]
// The kinds with a built prompt; the other questions of spells and abilities get theirs with their tasks.
const BUILT: DecisionName[] = [
  'YesNo',
  'ChooseOption',
  'ChooseMode',
  'ChooseCards',
  'OrderCards',
  'Mulligan',
  'BottomCards',
  'Priority',
  'PayMana',
  'DeclareAttackers',
  'DeclareBlockers',
  'AssignDamage',
  'Discard',
  'ChooseTargets',
  'OrderTriggers',
  'ChooseNumber',
  'ChooseReplacement',
  'DivideAmount',
  'ChooseCosts',
]

describe('the registry', () => {
  it('has a prompt for each decision kind and for nothing else', () => {
    expect(Object.keys(PROMPTS).toSorted()).toEqual([...NAMES].toSorted())
    expect(NAMES).toHaveLength(19)
  })

  it.each(BUILT)('has a built prompt for %s, not the stand-in', (name) => {
    expect(PROMPTS[name]).not.toBe(UnsupportedPrompt)
  })

  it('shows the prompt of Priority, which is built, at the fixture table', () => {
    const { container } = renderTable()
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(line).toHaveAttribute('data-decision', 'Priority')
    expect(line).not.toHaveTextContent('not supported')
  })
})

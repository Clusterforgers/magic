/** Goal: every prompt of a cast or activation that may still be backed out of (the way to cast it, modes, optional costs, X, targets, a division, cards for a cost, the payment) shows one Cancel button named for a spell or an ability, with the Esc hint, and both the button and Escape send the answer that backs out once; Escape typed into a field outside the prompt, or taken by another control, leaves the cast alone, and a prompt that may not be backed out of has no Cancel. */
import { fireEvent, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { Answer, DecisionKind } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { DEV_PROMPTS } from '../dev/prompts'

const fixture = fixtureOf(2)

/** The decision of the dev prompt `name`, with `can_cancel` set as given. */
function kindOf(name: string, canCancel = true): DecisionKind {
  const { kind } = must(DEV_PROMPTS[name])(fixture).decision
  const [[key, options]] = Object.entries(kind) as [[string, object]]
  return { [key]: { ...options, can_cancel: canCancel } } as DecisionKind
}

const CHAIN: [string, string, string, Answer][] = [
  ['way', 'ChooseOption', 'Cancel the spell', 'Undo'],
  ['mode', 'ChooseMode', 'Cancel the spell', 'Undo'],
  ['costs', 'ChooseCosts', 'Cancel the spell', 'Undo'],
  ['x', 'ChooseNumber', 'Cancel the spell', 'Undo'],
  ['targets', 'ChooseTargets', 'Cancel the spell', 'Undo'],
  ['divide', 'DivideAmount', 'Cancel the spell', 'Undo'],
  ['sacrifice-cost', 'ChooseCards', 'Cancel the ability', 'Undo'],
  ['pay', 'PayMana', 'Cancel the spell', 'Undo'],
]

describe('Cancel in the prompts of an announcement', () => {
  it.each(CHAIN)('%s (%s): the button backs out', async (name, kind, words, sent) => {
    const decision = kindOf(name)
    expect(Object.keys(decision)).toEqual([kind])
    const { answer } = renderDecision(decision)
    const [cancel] = screen.getAllByRole('button', { name: words })
    expect(screen.getAllByRole('button', { name: words })).toHaveLength(1)
    expect(cancel).toHaveAttribute('aria-keyshortcuts', 'Escape')
    expect(cancel).toHaveTextContent(/Esc$/)
    await userEvent.setup().click(screen.getByRole('button', { name: words }))
    expect(answer.mock.calls).toEqual([[sent]])
  })

  it.each(CHAIN)('%s (%s): Escape backs out', (name, _kind, _words, sent) => {
    const { answer } = renderDecision(kindOf(name))
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' })
    expect(answer.mock.calls).toEqual([[sent]])
  })

  it.each(CHAIN)('%s (%s): no Cancel when the cast may not be backed out of', (name) => {
    const { answer } = renderDecision(kindOf(name, false))
    expect(screen.queryByRole('button', { name: /^Cancel the (spell|ability)$/ })).toBeNull()
    fireEvent.keyDown(document.body, { key: 'Escape' })
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('Escape that belongs to something else', () => {
  it('typed into a field outside the prompt leaves the cast alone', () => {
    const { answer } = renderDecision(kindOf('targets'))
    const field = document.createElement('input')
    document.body.append(field)
    fireEvent.keyDown(field, { key: 'Escape' })
    field.remove()
    expect(answer).not.toHaveBeenCalled()
  })

  it('taken by another control first leaves the cast alone', () => {
    const { answer } = renderDecision(kindOf('pay'))
    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
    event.preventDefault()
    document.body.dispatchEvent(event)
    expect(answer).not.toHaveBeenCalled()
  })

  it('pressed in the X field still backs out', () => {
    const { answer } = renderDecision(kindOf('x'))
    fireEvent.keyDown(screen.getByRole('spinbutton'), { key: 'Escape' })
    expect(answer.mock.calls).toEqual([['Undo']])
  })
})

/** Goal: the yes-or-no prompt asks in the card's words (a plain question without them) with the keys for a screen reader, answers Yes or No from a click or from Y and N in either case exactly once, starts with focus on No (the answer the clock gives) so Enter at once says no, leaves the keys to a text field, and marks nothing on the table. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'

const source = must(creaturesOf(fixtureOf(2), 0)[0])
const sourceName = must(fixtureOf(2).view.objects.find((object) => object.id === source)).name

function mount(text?: string) {
  const table = renderDecision({ YesNo: text === undefined ? { source } : { source, text } })
  const yes = (): HTMLElement => screen.getByRole('button', { name: 'Yes' })
  const no = (): HTMLElement => screen.getByRole('button', { name: 'No' })
  return { ...table, yes, no }
}

describe('the question', () => {
  it.each([
    ['Draw a card?', `${sourceName}: Draw a card?`],
    ['Pay {2} to keep it?', `${sourceName}: Pay {2} to keep it?`],
    [undefined, `${sourceName}: do you want to do this?`],
  ])('with the words "%s" reads "%s" and tells a screen reader the keys', (text, sentence) => {
    const { text: said, container } = mount(text)
    expect(said()).toBe(`${sentence} Press Y for yes or N for no.`)
    expect(container.querySelector('[data-slot="prompt-text"]')).toHaveAttribute('aria-live', 'polite')
    expect(container.querySelector('[data-slot="prompt-title"]')).toHaveTextContent(
      'Your decision: Yes or no',
    )
  })

  it('draws Yes and No as the only controls, each with its key', () => {
    const { yes, no, container } = mount('Draw a card?')
    expect(
      screen.getAllByRole('button').filter((b) => container.contains(b) && b.closest('[data-decision]')),
    ).toHaveLength(2)
    expect(yes()).toHaveAttribute('aria-keyshortcuts', 'Y')
    expect(no()).toHaveAttribute('aria-keyshortcuts', 'N')
    expect(yes()).toHaveTextContent('YesY')
    expect(no()).toHaveTextContent('NoN')
  })

  it('marks no card on the table, since none can be chosen', () => {
    expect(mount('Draw a card?').marks().size).toBe(0)
  })
})

describe('answering', () => {
  it.each([
    ['a click on Yes', async (t: ReturnType<typeof mount>) => t.user.click(t.yes()), true],
    ['a click on No', async (t: ReturnType<typeof mount>) => t.user.click(t.no()), false],
    ['y', async (t: ReturnType<typeof mount>) => t.user.keyboard('y'), true],
    ['Y', async (t: ReturnType<typeof mount>) => t.user.keyboard('Y'), true],
    ['n', async (t: ReturnType<typeof mount>) => t.user.keyboard('n'), false],
    ['N', async (t: ReturnType<typeof mount>) => t.user.keyboard('N'), false],
  ])('%s sends the answer once', async (_how, gesture, said) => {
    const table = mount('Draw a card?')
    await gesture(table)
    expect(table.answer).toHaveBeenCalledTimes(1)
    expect(table.answer.mock.calls[0]?.[0]).toEqual({ YesNo: said })
  })

  it('sends one answer when the keys are pressed again', async () => {
    const { user, answer } = mount('Draw a card?')
    await user.keyboard('yny')
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ YesNo: true })
  })

  it('leaves y and n to a text field', async () => {
    const { user, answer } = mount('Draw a card?')
    const field = document.createElement('input')
    document.body.append(field)
    field.focus()
    await user.keyboard('yn')
    field.remove()
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('focus', () => {
  it('starts on No, so Enter at once answers what the clock would', async () => {
    const { no, user, answer } = mount('Draw a card?')
    expect(no()).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ YesNo: false })
  })

  it('takes Tab to Yes and Enter answers it', async () => {
    const { yes, user, answer } = mount('Draw a card?')
    await user.tab({ shift: true })
    expect(yes()).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(answer.mock.calls[0]?.[0]).toEqual({ YesNo: true })
  })
})

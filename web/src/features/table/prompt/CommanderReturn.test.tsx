/** Goal: a yes-or-no question the engine marks as a commander's return is the return choice: it asks "Return <name> to the command zone?", names where a no leaves the commander for each destination, makes Return the focused primary answer, answers from a click or Y and N, and leaves any other question as the plain Yes and No, a may of the asker's own commander included. */
import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import type { Fixture } from '@/features/table/dev/fixture'
import { DEV_TABLES } from '@/features/table/dev/prompts'
import type { Answer } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

const PLAIN = buildFixture({ seats: 4 })
const RETURN = must(DEV_TABLES['commander-return'])(PLAIN)

/** The viewer's second partner: in the command zone of `PLAIN`, in the graveyard of `RETURN`. */
const partner = (fixture: Fixture): number => must(fixture.view.players[0]?.commanders?.at(-1)).object
const nameOf = (fixture: Fixture, id: number): string =>
  must(fixture.view.objects.find((object) => object.id === id)).name

function mount(fixture: Fixture, source: number, text: string, commanderReturn = true) {
  const answer = vi.fn((_answer: Answer) => true)
  const user = userEvent.setup()
  const table = renderTable({
    fixture,
    session: { answer },
    state: (state) => ({
      ...state,
      pending: {
        id: 3,
        decision: { player: 0, kind: { YesNo: { source, text, commander_return: commanderReturn } } },
      },
    }),
  })
  const said = (): string =>
    must(table.container.querySelector('[data-slot="prompt-text"]')).textContent ?? ''
  return { ...table, answer, user, said }
}

describe('the return choice', () => {
  const name = nameOf(PLAIN, partner(PLAIN))
  it.each([
    [RETURN, `Put ${name} into the command zone?`, 'No leaves it in your graveyard.', 'Leave in graveyard'],
    [
      PLAIN,
      `Put ${name} into the command zone instead of a hand?`,
      'No puts it into your hand.',
      'Put into hand',
    ],
    [
      PLAIN,
      `Put ${name} into the command zone instead of a library?`,
      'No puts it into your library.',
      'Put into library',
    ],
    [
      PLAIN,
      `Put ${name} into the command zone instead of exile?`,
      'No leaves it in exile.',
      'Leave in exile',
    ],
    [
      PLAIN,
      `Put ${name} into the command zone instead of a graveyard?`,
      'No leaves it in your graveyard.',
      'Leave in graveyard',
    ],
  ])('asks about %#: %s', (fixture, text, detail, noLabel) => {
    const { said } = mount(fixture, partner(fixture), text)
    expect(said()).toBe(`Return ${name} to the command zone? ${detail} Press Y for yes or N for no.`)
    expect(screen.getByRole('button', { name: 'Return to the command zone' })).toHaveFocus()
    expect(screen.getByRole('button', { name: noLabel })).toHaveAttribute('aria-keyshortcuts', 'N')
  })

  it.each([
    ['Return', true],
    ['Leave in graveyard', false],
  ])('a click on %s answers %s once', async (label, yes) => {
    const table = mount(RETURN, partner(RETURN), `Put ${name} into the command zone?`)
    await table.user.click(
      screen.getByRole('button', { name: label === 'Return' ? 'Return to the command zone' : label }),
    )
    expect(table.answer).toHaveBeenCalledTimes(1)
    expect(table.answer.mock.calls[0]?.[0]).toEqual({ YesNo: yes })
  })

  it.each([
    ['y', true],
    ['n', false],
  ])('the key %s answers %s', async (key, yes) => {
    const table = mount(RETURN, partner(RETURN), `Put ${name} into the command zone?`)
    await table.user.keyboard(key)
    expect(table.answer.mock.calls[0]?.[0]).toEqual({ YesNo: yes })
  })

  it.each([
    ['another seat', (fixture: Fixture) => must(fixture.view.players[1]?.commanders?.[0]).object],
    ["the asker's own", partner],
  ])("asks a may of %s commander as a plain question in the card's words", (_, source) => {
    const { said } = mount(RETURN, source(RETURN), 'Draw a card?', false)
    expect(said()).not.toContain('command zone')
    expect(screen.getByRole('button', { name: 'Yes' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'No' })).toHaveFocus()
  })

  it('keeps the answer row sticky at the sheet bottom, so the buttons stay in view while the sheet scrolls', () => {
    const { container } = mount(RETURN, partner(RETURN), 'Put it into the command zone?')
    expect(container.querySelector('[data-slot="prompt-actions"]')).toHaveClass('sticky', 'bottom-0')
  })
})

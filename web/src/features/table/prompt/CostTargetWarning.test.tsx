/** Goal: in the cost pick (`CardsOptions.targeted`) and the target pick (`TargetOptions.paid`), an object that is both a target and paid for the cost is badged in the grid and on the table, and an answer that takes it shows a warning first, whose "anyway" button sends the answer unchanged and whose "Choose another" (focused, also Escape, never the prompt's Cancel) returns to the choice; an answer that takes no such object is sent at once. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, Target } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { PAYS_COST_BADGE, TARGETED_BADGE } from './costTarget'

const fixture = fixtureOf(2)
const mine = creaturesOf(fixture, 0).slice(0, 3)
const theirs = creaturesOf(fixture, 1).slice(0, 3)
const source = must(mine[2])
const clash = must(mine[0])
const fine = must(mine[1])
const sourceName = must(fixture.view.objects.find((object) => object.id === source)).name
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((object) => object.id === id)).name
const object = (id: ObjectId): Target => ({ Object: { id, counter: 0 } })

/** The two prompts, each with the press that makes an answer of `id` and the words of its "anyway" button. */
const PROMPTS = [
  {
    name: 'the cost pick',
    anyway: 'Pay anyway',
    badge: TARGETED_BADGE,
    mount: () =>
      renderDecision({
        ChooseCards: {
          source,
          from: [clash, fine],
          min: 1,
          max: 1,
          text: 'Sacrifice a creature.',
          can_cancel: true,
          targeted: [clash],
        },
      }),
    async choose(table: ReturnType<typeof renderDecision>, id: ObjectId) {
      await table.user.click(table.card(id))
      await table.user.click(table.button('Confirm'))
    },
    sent: (id: ObjectId) => ({ ChooseCards: [id] }),
  },
  {
    name: 'the target pick',
    anyway: 'Target anyway',
    badge: PAYS_COST_BADGE,
    mount: () =>
      renderDecision({
        ChooseTargets: {
          source,
          spec: 0,
          legal: [clash, fine, ...theirs].map(object),
          min: 1,
          max: 1,
          can_cancel: true,
          wording: 'any target',
          paid: [clash],
        },
      }),
    async choose(table: ReturnType<typeof renderDecision>, id: ObjectId) {
      await table.user.click(table.card(id))
    },
    sent: (id: ObjectId) => ({ ChooseTargets: { Targets: [object(id)] } }),
  },
]

describe.each(PROMPTS)('$name', (prompt) => {
  it('badges the object on the table', () => {
    const table = prompt.mount()
    expect(table.notes().get(clash)).toBe(prompt.badge)
    expect(table.notes().has(fine)).toBe(false)
  })

  it('sends an answer that takes no such object at once', async () => {
    const table = prompt.mount()
    await prompt.choose(table, fine)
    expect(table.answer).toHaveBeenCalledWith(prompt.sent(fine))
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('warns before the clashing answer and sends it unchanged on "anyway"', async () => {
    const table = prompt.mount()
    await prompt.choose(table, clash)
    expect(table.answer).not.toHaveBeenCalled()
    expect(table.text()).toBe(
      `${sourceName}'s target leaves the battlefield: ${nameOf(clash)} pays the cost. It will have no target.`,
    )
    expect(screen.getByRole('alert')).toBeTruthy()
    expect(document.activeElement).toBe(table.button(/Choose another/))
    expect(screen.queryByRole('button', { name: /Cancel the/ })).toBeNull()
    await table.user.click(table.button(prompt.anyway))
    expect(table.answer).toHaveBeenCalledTimes(1)
    expect(table.answer).toHaveBeenCalledWith(prompt.sent(clash))
  })

  it.each([
    ['the button', 'click'],
    ['Escape', 'escape'],
    ['Enter on the focused button', 'enter'],
  ])('"Choose another" by %s returns to the choice without an answer', async (_how, how) => {
    const table = prompt.mount()
    await prompt.choose(table, clash)
    if (how === 'click') await table.user.click(table.button(/Choose another/))
    else await table.user.keyboard(how === 'escape' ? '{Escape}' : '{Enter}')
    expect(table.answer).not.toHaveBeenCalled()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(screen.queryByRole('button', { name: /Cancel the/ })).not.toBeNull()
  })
})

describe('the cost pick grid', () => {
  it('reads the badge after the card name', () => {
    renderDecision({
      ChooseCards: { source, from: [clash, fine], min: 1, max: 1, targeted: [clash] },
    })
    const badged = document.querySelectorAll('[data-candidate] [data-slot="card-badge"]')
    expect(badged).toHaveLength(1)
    expect(must(badged[0]).textContent).toBe(TARGETED_BADGE)
    expect(must(badged[0]).closest('button')?.getAttribute('aria-label')).toMatch(/, targeted$/)
  })
})

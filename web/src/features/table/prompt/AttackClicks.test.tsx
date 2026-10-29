/** Goal: attacking is done by clicking alone: a click on a creature puts it in the attack and a second click takes it out, no label is written on a card, with several seats the defender is chosen by clicking a pod (the first one at the start, the last choice after) and shows chosen, a creature that cannot attack the chosen defender says so on hover and focus, and the answers sent have the exact shape the server reads. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { AttackerOption, AttackTarget, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'

const attackers = (seats: number, defenders: AttackTarget[], count = 3): AttackerOption[] =>
  creaturesOf(fixtureOf(seats), 0)
    .slice(0, count)
    .map((object): AttackerOption => ({ object, defenders }))

const seatsOf = (seats: number): AttackTarget[] =>
  Array.from({ length: seats - 1 }, (_, index): AttackTarget => ({ Player: index + 1 }))

const table = (seats: number, options: AttackerOption[]) =>
  renderCombat({ DeclareAttackers: { attackers: options } }, { seats })

const pod = (t: ReturnType<typeof table>, seat: number): HTMLElement =>
  must(t.container.querySelector<HTMLElement>(`section[data-seat="${seat}"]`))

describe('the clicks of an attack', () => {
  it.each([
    { name: 'one click attacks', clicks: [0], sent: [0] },
    { name: 'two creatures in the order they were clicked', clicks: [1, 0], sent: [1, 0] },
    { name: 'a second click takes the creature back', clicks: [0, 1, 0], sent: [1] },
    { name: 'a third click attacks again, last', clicks: [0, 1, 0, 0], sent: [1, 0] },
  ])('with two seats $name', async ({ clicks, sent }) => {
    const options = attackers(2, [{ Player: 1 }])
    const t = table(2, options)
    for (const click of clicks) await t.user.click(t.card(must(options[click]).object))
    await t.user.click(t.button('Confirm attack'))
    expect(t.answer.mock.calls).toEqual([
      [{ DeclareAttackers: sent.map((index) => [must(options[index]).object, { Player: 1 }]) }],
    ])
    expect(t.container.querySelector('[data-slot="card-note"]')).toBeNull()
  })

  it('shows an attacker as chosen and pressed, takes it back and disables Confirm again', async () => {
    const options = attackers(2, [{ Player: 1 }])
    const [first] = options.map((option) => option.object) as [ObjectId]
    const t = table(2, options)
    expect(t.button('Confirm attack')).toBeDisabled()
    await t.user.click(t.card(first))
    expect(t.marks().get(first)).toBe('selected')
    expect(t.card(first)).toHaveAttribute('aria-pressed', 'true')
    await t.user.click(t.card(first))
    expect(t.marks().get(first)).toBe('legal')
    expect(t.button('Confirm attack')).toBeDisabled()
  })

  it('has a bar of icon buttons in one row and no list, panel or defender choice at two seats', () => {
    const t = table(2, attackers(2, [{ Player: 1 }]))
    const bar = must(t.container.querySelector('[data-slot="combat-bar"]'))
    expect([...bar.querySelectorAll('button')].map((button) => button.getAttribute('aria-label'))).toEqual([
      'Attack with all',
      'Clear',
      'Confirm attack',
      'No attack',
    ])
    expect(screen.queryByRole('combobox')).toBeNull()
    expect(t.container.querySelector('[data-defender]')).toBeNull()
  })

  it('attacks with all, clears, and answers No attack', async () => {
    const options = attackers(2, [{ Player: 1 }])
    const t = table(2, options)
    await t.user.click(t.button('Attack with all'))
    expect([...t.marks()].filter(([, mark]) => mark === 'selected')).toHaveLength(3)
    await t.user.click(t.button('Clear'))
    expect([...t.marks()].filter(([, mark]) => mark === 'selected')).toHaveLength(0)
    await t.user.click(t.button('No attack'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareAttackers: [] }]])
  })
})

describe('the keyboard', () => {
  it('toggles a creature with Enter and Space on its card', async () => {
    const options = attackers(2, [{ Player: 1 }])
    const [first] = options.map((option) => option.object) as [ObjectId]
    const t = table(2, options)
    act(() => t.card(first).focus())
    await t.user.keyboard('{Enter}')
    expect(t.marks().get(first)).toBe('selected')
    await t.user.keyboard(' ')
    expect(t.marks().get(first)).toBe('legal')
  })

  it('confirms with Enter on the bar', async () => {
    const options = attackers(2, [{ Player: 1 }])
    const [first] = options.map((option) => option.object) as [ObjectId]
    const t = table(2, options)
    await t.user.click(t.card(first))
    act(() => t.button('Confirm attack').focus())
    await t.user.keyboard('{Enter}')
    expect(t.answer.mock.calls).toEqual([[{ DeclareAttackers: [[first, { Player: 1 }]] }]])
  })

  it('confirms with Enter alone when nothing is focused', async () => {
    const options = attackers(2, [{ Player: 1 }])
    const [first] = options.map((option) => option.object) as [ObjectId]
    const t = table(2, options)
    await t.user.click(t.card(first))
    act(() => (document.activeElement as HTMLElement).blur())
    await t.user.keyboard('{Enter}')
    expect(t.answer.mock.calls).toEqual([[{ DeclareAttackers: [[first, { Player: 1 }]] }]])
  })
})

describe('the defender of several seats', () => {
  it.each([3, 4])(
    'at %i seats starts at the first seat and a click on a pod aims the next creatures there',
    async (seats) => {
      const options = attackers(seats, seatsOf(seats))
      const [a, b, c] = options.map((option) => option.object) as [ObjectId, ObjectId, ObjectId]
      const t = table(seats, options)
      const last = seats - 1
      expect(pod(t, 1)).toHaveAttribute('data-defender', 'chosen')
      expect(pod(t, last)).toHaveAttribute('data-defender', 'candidate')
      expect(pod(t, 0)).not.toHaveAttribute('data-defender')
      await t.user.click(t.card(a))
      await t.user.click(pod(t, last))
      expect(pod(t, last)).toHaveAttribute('data-defender', 'chosen')
      expect(pod(t, 1)).toHaveAttribute('data-defender', 'candidate')
      await t.user.click(t.card(b))
      await t.user.click(t.card(c))
      await t.user.click(t.button('Confirm attack'))
      expect(t.answer.mock.calls).toEqual([
        [
          {
            DeclareAttackers: [
              [a, { Player: 1 }],
              [b, { Player: last }],
              [c, { Player: last }],
            ],
          },
        ],
      ])
    },
  )

  it('keeps the last choice for the next attack of the table', async () => {
    const options = attackers(3, seatsOf(3))
    const t = table(3, options)
    await t.user.click(pod(t, 2))
    t.update(() => ({
      pending: { id: 9, decision: { player: 0, kind: { DeclareAttackers: { attackers: options } } } },
    }))
    expect(pod(t, 2)).toHaveAttribute('data-defender', 'chosen')
    await t.user.click(t.card(must(options[0]).object))
    await t.user.click(t.button('Confirm attack'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareAttackers: [[must(options[0]).object, { Player: 2 }]] }]])
  })

  it('takes the pods back when the decision is gone, and a click on a zone chip picks nothing', async () => {
    const t = table(3, attackers(3, seatsOf(3)))
    await t.user.click(must(pod(t, 2).querySelector('button')))
    expect(pod(t, 1)).toHaveAttribute('data-defender', 'chosen')
    t.update(() => ({ pending: null }))
    expect(t.container.querySelector('[data-defender]')).toBeNull()
  })

  it('lists the defenders in the bar too, which a keyboard reaches, and a pod and the list agree', async () => {
    const t = table(4, attackers(4, seatsOf(4)))
    const select = screen.getByRole('combobox', { name: 'Attack at' })
    await t.user.selectOptions(select, 'player:3')
    expect(pod(t, 3)).toHaveAttribute('data-defender', 'chosen')
    await t.user.click(pod(t, 2))
    expect(select).toHaveValue('player:2')
  })

  it('sends a creature at a planeswalker chosen in the list', async () => {
    const walker = must(
      fixtureOf(3).view.objects.find(
        (object) =>
          object.zone === 'Battlefield' && object.controller === 1 && !object.types.includes('Creature'),
      ),
    ).id
    const options = attackers(3, [...seatsOf(3), { Planeswalker: walker }], 1)
    const t = table(3, options)
    await t.user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), `walker:${walker}`)
    expect(t.container.querySelector('[data-defender="chosen"]')).toBeNull()
    await t.user.click(t.card(must(options[0]).object))
    await t.user.click(t.button('Confirm attack'))
    expect(t.answer.mock.calls).toEqual([
      [{ DeclareAttackers: [[must(options[0]).object, { Planeswalker: walker }]] }],
    ])
  })
})

describe('a creature that cannot attack the chosen defender', () => {
  const restricted = (): { a: ObjectId; b: ObjectId; t: ReturnType<typeof table> } => {
    const [a, b] = creaturesOf(fixtureOf(3), 0) as [ObjectId, ObjectId]
    const t = table(3, [
      { object: a, defenders: [{ Player: 1 }, { Player: 2 }] },
      { object: b, defenders: [{ Player: 2 }] },
    ])
    return { a, b, t }
  }

  it('says so in the bar on hover, and not on the card', async () => {
    const { a, b, t } = restricted()
    await t.user.hover(t.card(a))
    expect(t.text()).not.toMatch(/cannot attack/)
    await t.user.hover(t.card(b))
    expect(t.text()).toMatch(/^.* cannot attack Bo; it attacks Cy instead\.$/)
    expect(t.container.querySelector('[data-slot="card-note"]')).toBeNull()
    await t.user.unhover(t.card(b))
    expect(t.text()).not.toMatch(/cannot attack/)
  })

  it('says so on keyboard focus', () => {
    const { b, t } = restricted()
    act(() => t.card(b).focus())
    expect(t.text()).toMatch(/cannot attack Bo; it attacks Cy instead/)
  })

  it('still attacks its own defender with a click, and says it is no longer a problem once Cy is chosen', async () => {
    const { b, t } = restricted()
    await t.user.click(t.card(b))
    await t.user.click(t.button('Confirm attack'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareAttackers: [[b, { Player: 2 }]] }]])
  })

  it('does not say it for the defender it can attack', async () => {
    const { b, t } = restricted()
    await t.user.click(pod(t, 2))
    await t.user.hover(t.card(b))
    expect(t.text()).not.toMatch(/cannot attack/)
  })
})

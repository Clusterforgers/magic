/** Goal: for every kind of attack the options can offer (no attackers, one defender, many defenders at 2 to 12 seats, a planeswalker, attackers with different defenders) exactly the creatures that may attack are marked legal and the rest of the viewer's cards dimmed; a click, Enter or Space puts a creature in the attack or takes it out, each creature goes where "Attack at" says (automatically when there is one defender) and can be re-aimed, the card, the summary and the answer agree, Clear, No attack and Confirm answer once, and a replaced decision starts again empty. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { AttackerOption, AttackOptions, AttackTarget, ObjectId } from '@/protocol/generated'
import { creaturesOf, fixtureOf, ownOf, renderCombat } from '@/test-utils/combatTable'
import { must } from '@/test-utils/batches'
import { NAMES } from '../dev/fixture'

const seatsOf = (seats: number): AttackTarget[] =>
  Array.from({ length: seats - 1 }, (_, index): AttackTarget => ({ Player: index + 1 }))

/** A permanent of seat 1 that stands in for a planeswalker, since the fixture has none. */
const walkerOf = (seats: number): ObjectId =>
  must(
    fixtureOf(seats).view.objects.find(
      (object) =>
        object.zone === 'Battlefield' && object.controller === 1 && !object.types.includes('Creature'),
    ),
  ).id

interface Spec {
  name: string
  seats: number
  /** How many of the viewer's creatures may attack. */
  count: number
  walker?: boolean
}

const SPECS: Spec[] = [
  { name: 'no attackers', seats: 2, count: 0 },
  { name: 'one attacker and one defender', seats: 2, count: 1 },
  { name: 'five attackers and one defender', seats: 2, count: 5 },
  { name: 'three attackers and two defenders', seats: 3, count: 3 },
  { name: 'six attackers and three defenders', seats: 4, count: 6 },
  { name: 'six attackers, three defenders and a planeswalker', seats: 4, count: 6, walker: true },
  { name: 'eight attackers and seven defenders', seats: 8, count: 8 },
  { name: 'nine attackers and eleven defenders', seats: 12, count: 9 },
]

function optionsOf({ seats, count, walker = false }: Spec): AttackOptions {
  const defenders = [...seatsOf(seats), ...(walker ? [{ Planeswalker: walkerOf(seats) }] : [])]
  return {
    attackers: creaturesOf(fixtureOf(seats), 0)
      .slice(0, count)
      .map((object): AttackerOption => ({ object, defenders })),
  }
}

const attack = (options: AttackOptions, seats = 2, id = 7) =>
  renderCombat({ DeclareAttackers: options }, { seats, id })

describe('what is marked', () => {
  it.each(SPECS)('with $name the attackers are legal and the rest of the viewer dimmed', (spec) => {
    const options = optionsOf(spec)
    const { marks, fixture } = attack(options, spec.seats)
    const legal = new Set(options.attackers.map((a) => a.object))
    expect(marks()).toEqual(new Map(ownOf(fixture).map((id) => [id, legal.has(id) ? 'legal' : 'dimmed'])))
  })

  it('marks an attacker that has no defender as not available', () => {
    const [first, second] = creaturesOf(fixtureOf(2), 0) as [ObjectId, ObjectId]
    const { marks } = attack({
      attackers: [
        { object: first, defenders: [{ Player: 1 }] },
        { object: second, defenders: [] },
      ],
    })
    expect(marks().get(first)).toBe('legal')
    expect(marks().get(second)).toBe('dimmed')
  })

  it('says in the name of a card that it is available', () => {
    const options = optionsOf(SPECS[1] as Spec)
    const { card, fixture } = attack(options)
    const other = must(creaturesOf(fixture, 0)[1])
    expect(card(must(options.attackers[0]).object)).toHaveAccessibleName(/, available$/)
    expect(card(other)).toHaveAccessibleName(/, not available$/)
  })
})

describe('one defender', () => {
  const spec: Spec = { name: '', seats: 2, count: 4 }
  const [a, b, c] = optionsOf(spec).attackers.map((x) => x.object) as [ObjectId, ObjectId, ObjectId]

  it('toggles a creature with a click, and shows it pressed', async () => {
    const { card, user, marks } = attack(optionsOf(spec))
    await user.click(card(a))
    expect(marks().get(a)).toBe('selected')
    expect(card(a)).toHaveAttribute('aria-pressed', 'true')
    expect(card(b)).toHaveAttribute('aria-pressed', 'false')
    await user.click(card(a))
    expect(marks().get(a)).toBe('legal')
    expect(card(a)).toHaveAttribute('aria-pressed', 'false')
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('toggles a creature with %s', async (_key, keys) => {
    const { card, user, marks } = attack(optionsOf(spec))
    act(() => card(a).focus())
    await user.keyboard(keys)
    expect(marks().get(a)).toBe('selected')
    await user.keyboard(keys)
    expect(marks().get(a)).toBe('legal')
  })

  it('ignores a dimmed creature', async () => {
    const dimmed = must(creaturesOf(fixtureOf(2), 0)[8])
    const { card, user, marks } = attack(optionsOf(spec))
    await user.click(card(dimmed))
    expect(marks().get(dimmed)).toBe('dimmed')
  })

  it('has no "Attack at" choice and no list, and aims every attacker at the one defender', async () => {
    const { card, user, button, answer } = attack(optionsOf(spec))
    expect(screen.queryByRole('combobox')).toBeNull()
    await user.click(card(b))
    await user.click(card(a))
    expect(screen.queryByRole('list', { name: 'Who each creature attacks' })).toBeNull()
    await user.click(button('Confirm attack'))
    expect(answer.mock.calls).toEqual([
      [
        {
          DeclareAttackers: [
            [b, { Player: 1 }],
            [a, { Player: 1 }],
          ],
        },
      ],
    ])
  })

  it('writes "At Bo" on a chosen attacker and speaks it', async () => {
    const { card, user, notes } = attack(optionsOf(spec))
    await user.click(card(a))
    expect(notes()).toEqual(new Map([[a, 'At Bo']]))
    expect(card(a)).toHaveAccessibleName(/, attacking Bo$/)
    await user.click(card(a))
    expect(notes().size).toBe(0)
  })

  it('sums up the attack in words as it grows', async () => {
    const { card, user, text } = attack(optionsOf(spec))
    expect(text()).toBe('Choose the creatures that attack Bo, or choose No attack.')
    await user.click(card(a))
    expect(text()).toBe('1 attacker at Bo. Confirm the attack, or keep choosing.')
    await user.click(card(b))
    await user.click(card(c))
    expect(text()).toBe('3 attackers at Bo. Confirm the attack, or keep choosing.')
  })

  it('attacks with all, in the order the options list them', async () => {
    const options = optionsOf(spec)
    const { user, button, answer } = attack(options)
    await user.click(button('Attack with all'))
    await user.click(button('Confirm attack'))
    expect(answer.mock.calls).toEqual([
      [
        {
          DeclareAttackers: options.attackers.map((x): [ObjectId, AttackTarget] => [x.object, { Player: 1 }]),
        },
      ],
    ])
  })

  it('keeps the creatures already chosen in front when attacking with all', async () => {
    const { card, user, button, answer } = attack(optionsOf(spec))
    await user.click(card(c))
    await user.click(button('Attack with all'))
    await user.click(button('Confirm attack'))
    const sent = answer.mock.calls[0]?.[0] as { DeclareAttackers: [ObjectId, AttackTarget][] }
    expect(sent.DeclareAttackers.map(([id]) => id)).toEqual([
      c,
      a,
      b,
      must(optionsOf(spec).attackers[3]).object,
    ])
  })

  it('clears the attack, which disables Confirm attack', async () => {
    const { card, user, button, marks } = attack(optionsOf(spec))
    expect(button('Confirm attack')).toBeDisabled()
    expect(button('Clear')).toBeDisabled()
    await user.click(card(a))
    await user.click(card(b))
    expect(button('Confirm attack')).toBeEnabled()
    await user.click(button('Clear'))
    expect(marks().get(a)).toBe('legal')
    expect(button('Confirm attack')).toBeDisabled()
  })

  it('answers an empty attack with No attack, even with creatures chosen', async () => {
    const { card, user, button, answer } = attack(optionsOf(spec))
    await user.click(card(a))
    await user.click(button('No attack'))
    expect(answer.mock.calls).toEqual([[{ DeclareAttackers: [] }]])
  })

  it.each([
    ['Confirm attack', async (t: ReturnType<typeof attack>) => t.user.dblClick(t.button('Confirm attack'))],
    ['No attack', async (t: ReturnType<typeof attack>) => t.user.dblClick(t.button('No attack'))],
  ])('sends one answer for a double click on %s', async (_name, gesture) => {
    const table = attack(optionsOf(spec))
    await table.user.click(table.card(a))
    await gesture(table)
    expect(table.answer).toHaveBeenCalledTimes(1)
  })

  it('sends one answer for Enter pressed twice on Confirm attack', async () => {
    const { card, user, button, answer } = attack(optionsOf(spec))
    await user.click(card(a))
    act(() => button('Confirm attack').focus())
    await user.keyboard('{Enter}{Enter}')
    expect(answer).toHaveBeenCalledTimes(1)
  })
})

describe('no attackers', () => {
  it('says so, offers only No attack and answers an empty attack', async () => {
    const { text, button, user, answer } = attack({ attackers: [] })
    expect(text()).toBe('None of your creatures can attack. Choose No attack to go on.')
    expect(button('Attack with all')).toBeDisabled()
    expect(button('Confirm attack')).toBeDisabled()
    await user.click(button('No attack'))
    expect(answer.mock.calls).toEqual([[{ DeclareAttackers: [] }]])
  })
})

describe('several defenders', () => {
  const seatCases: [number, string][] = [
    [3, 'Cy'],
    [4, 'Dee'],
    [8, 'Hana'],
    [12, 'Lena'],
  ]
  it.each(seatCases)('at %i seats lists every other seat and starts with the first', (seats) => {
    attack(optionsOf({ name: '', seats, count: 3 }), seats)
    const select = screen.getByRole('combobox', { name: 'Attack at' })
    expect(
      within(select)
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(NAMES.slice(1, seats))
    expect(select).toHaveValue('player:1')
  })

  it.each(seatCases)(
    'at %i seats a click aims at the chosen seat (%s) and the answer agrees',
    async (seats, last) => {
      const options = optionsOf({ name: '', seats, count: 3 })
      const table = attack(options, seats)
      const [a, b, c] = options.attackers.map((x) => x.object) as [ObjectId, ObjectId, ObjectId]
      await table.user.click(table.card(a))
      await table.user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), last)
      await table.user.click(table.card(b))
      await table.user.click(table.card(c))
      expect(table.notes()).toEqual(
        new Map([
          [a, 'At Bo'],
          [b, `At ${last}`],
          [c, `At ${last}`],
        ]),
      )
      expect(table.text()).toBe(`3 attackers: 1 at Bo, 2 at ${last}. Confirm the attack, or keep choosing.`)
      await table.user.click(table.button('Confirm attack'))
      expect(table.answer.mock.calls).toEqual([
        [
          {
            DeclareAttackers: [
              [a, { Player: 1 }],
              [b, { Player: seats - 1 }],
              [c, { Player: seats - 1 }],
            ],
          },
        ],
      ])
    },
  )

  it('shows one row per attacker and lets one be aimed elsewhere', async () => {
    const options = optionsOf({ name: '', seats: 4, count: 3 })
    const table = attack(options, 4)
    const [a, b] = options.attackers.map((x) => x.object) as [ObjectId, ObjectId]
    await table.user.click(table.card(a))
    await table.user.click(table.card(b))
    const list = screen.getByRole('list', { name: 'Who each creature attacks' })
    expect(within(list).getAllByRole('listitem')).toHaveLength(2)
    const rows = within(list).getAllByRole('combobox')
    await table.user.selectOptions(must(rows[1]), 'player:3')
    expect(table.text()).toBe('2 attackers: 1 at Bo, 1 at Dee. Confirm the attack, or keep choosing.')
    expect(table.notes().get(b)).toBe('At Dee')
    await table.user.click(table.button('Confirm attack'))
    expect(table.answer.mock.calls).toEqual([
      [
        {
          DeclareAttackers: [
            [a, { Player: 1 }],
            [b, { Player: 3 }],
          ],
        },
      ],
    ])
  })

  it('aims everything chosen at one seat with "All at"', async () => {
    const options = optionsOf({ name: '', seats: 4, count: 4 })
    const table = attack(options, 4)
    expect(screen.queryByRole('button', { name: /^All at/ })).toBeNull()
    await table.user.click(table.button('Attack with all'))
    expect(table.text()).toBe('4 attackers at Bo. Confirm the attack, or keep choosing.')
    await table.user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), 'player:2')
    await table.user.click(table.button('All at Cy'))
    expect(table.text()).toBe('4 attackers at Cy. Confirm the attack, or keep choosing.')
    await table.user.click(table.button('Confirm attack'))
    const sent = table.answer.mock.calls[0]?.[0] as { DeclareAttackers: [ObjectId, AttackTarget][] }
    expect(sent.DeclareAttackers.map(([, target]) => target)).toEqual(Array(4).fill({ Player: 2 }))
  })

  it('attacks with all at the chosen seat', async () => {
    const table = attack(optionsOf({ name: '', seats: 12, count: 5 }), 12)
    await table.user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), 'player:11')
    await table.user.click(table.button('Attack with all'))
    expect(table.text()).toBe('5 attackers at Lena. Confirm the attack, or keep choosing.')
  })

  it('says what to do before anything is chosen', () => {
    const { text } = attack(optionsOf({ name: '', seats: 4, count: 3 }), 4)
    expect(text()).toBe('Choose the creatures that attack and who each one attacks, or choose No attack.')
  })

  it('offers a planeswalker by name and owner, and answers with it', async () => {
    const options = optionsOf({ name: '', seats: 4, count: 2, walker: true })
    const table = attack(options, 4)
    const walker = walkerOf(4)
    const select = screen.getByRole('combobox', { name: 'Attack at' })
    const labels = within(select)
      .getAllByRole('option')
      .map((o) => o.textContent)
    expect(labels.at(-1)).toMatch(/, Bo's planeswalker$/)
    await table.user.selectOptions(select, `walker:${walker}`)
    const [a] = options.attackers.map((x) => x.object) as [ObjectId]
    await table.user.click(table.card(a))
    expect(table.notes().get(a)).toMatch(/^At /)
    expect(table.card(a)).toHaveAccessibleName(/, attacking .*, Bo's planeswalker$/)
    await table.user.click(table.button('Confirm attack'))
    expect(table.answer.mock.calls).toEqual([[{ DeclareAttackers: [[a, { Planeswalker: walker }]] }]])
  })

  it('aims an attacker that cannot attack the chosen seat at its own first defender', async () => {
    const [a, b] = creaturesOf(fixtureOf(4), 0) as [ObjectId, ObjectId]
    const table = attack(
      {
        attackers: [
          { object: a, defenders: [{ Player: 1 }, { Player: 2 }, { Player: 3 }] },
          { object: b, defenders: [{ Player: 2 }, { Player: 3 }] },
        ],
      },
      4,
    )
    await table.user.click(table.card(a))
    await table.user.click(table.card(b))
    expect(table.notes()).toEqual(
      new Map([
        [a, 'At Bo'],
        [b, 'At Cy'],
      ]),
    )
    const [first, second] = screen.getAllByRole('combobox', { name: /^Who / })
    expect(within(must(first)).getAllByRole('option')).toHaveLength(3)
    expect(within(must(second)).getAllByRole('option')).toHaveLength(2)
  })

  it('tells creatures of the same name apart in the list', async () => {
    const options = optionsOf({ name: '', seats: 4, count: 3 })
    const table = attack(options, 4)
    const [a, b, c] = options.attackers.map((x) => x.object) as [ObjectId, ObjectId, ObjectId]
    table.update((state) => {
      const named = (id: ObjectId, name: string) => ({ ...must(state.objectsById[id]), name })
      return {
        objectsById: {
          ...state.objectsById,
          [a]: named(a, 'Goblin'),
          [b]: named(b, 'Goblin'),
          [c]: named(c, 'Elf'),
        },
      }
    })
    await table.user.click(table.button('Attack with all'))
    expect(
      screen.getAllByRole('combobox', { name: /^Who / }).map((select) => select.getAttribute('aria-label')),
    ).toEqual(['Who Goblin 1 attacks', 'Who Goblin 2 attacks', 'Who Elf attacks'])
  })

  it('aims a creature at the seat that was chosen before it', async () => {
    const options = optionsOf({ name: '', seats: 4, count: 2 })
    const table = attack(options, 4)
    await table.user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), 'player:2')
    const [a] = options.attackers.map((x) => x.object) as [ObjectId]
    await table.user.click(table.card(a))
    expect(table.notes().get(a)).toBe('At Cy')
  })
})

describe('a replaced decision', () => {
  it('forgets the attack and the aim, and marks the new options', async () => {
    const options = optionsOf({ name: '', seats: 4, count: 3 })
    const table = attack(options, 4)
    const [a, b, c] = options.attackers.map((x) => x.object) as [ObjectId, ObjectId, ObjectId]
    await table.user.click(table.card(a))
    await table.user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), 'player:3')
    expect(table.marks().get(a)).toBe('selected')
    table.update((state) => ({
      pending: {
        id: 8,
        decision: {
          player: 0,
          kind: {
            DeclareAttackers: { attackers: [{ object: b, defenders: [{ Player: 1 }, { Player: 2 }] }] },
          },
        },
      },
      seq: state.seq,
    }))
    expect(table.marks().get(a)).toBe('dimmed')
    expect(table.marks().get(b)).toBe('legal')
    expect(table.marks().get(c)).toBe('dimmed')
    expect(table.notes().size).toBe(0)
    expect(screen.getByRole('combobox', { name: 'Attack at' })).toHaveValue('player:1')
    expect(table.button('Confirm attack')).toBeDisabled()
  })

  it('takes every mark and note away once the decision is gone', async () => {
    const options = optionsOf({ name: '', seats: 2, count: 2 })
    const table = attack(options)
    await table.user.click(table.card(must(options.attackers[0]).object))
    expect(table.notes().size).toBe(1)
    table.update(() => ({ pending: null }))
    expect(table.marks().size).toBe(0)
    expect(table.notes().size).toBe(0)
    expect(table.container.querySelector('[data-decision]')).toBeNull()
  })
})

describe('the words', () => {
  it('is one polite live region under the name of the decision', () => {
    const { container } = attack(optionsOf({ name: '', seats: 2, count: 2 }))
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(within(line).getByText('Your decision: Declare attackers')).toBeInTheDocument()
    expect(line.querySelectorAll('[aria-live]')).toHaveLength(1)
  })
})

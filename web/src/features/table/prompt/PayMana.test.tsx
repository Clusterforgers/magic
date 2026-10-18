/** Goal: for many costs and source sets (generic, coloured, colourless, floating mana, several colours on one permanent, too little mana) the PayMana prompt marks exactly the sources that still help as legal, builds the payment from clicks and keys in order, enables Confirm only for a payment the engine would accept and sends it as one Manual answer; Pay automatically and Cancel answer once, a chosen source can be taken back, and a new decision starts clean. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId, PayOptions, Payment } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { cost, manaOfLetter, poolOf, source } from '@/test-utils/mana'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const view = fixture.view
const byId = new Map(view.objects.map((object) => [object.id, object]))
const hand = must(view.players[0]?.hand.cards)
const battlefield = view.objects.filter((object) => object.zone === 'Battlefield' && object.controller === 0)
const OWN: ObjectId[] = [...hand, ...battlefield.map((object) => object.id)]
const lands = battlefield
  .filter((object) => object.types.includes('Land') && object.permanent?.tapped === false)
  .map((object) => object.id)
const evangel = must(view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === 0))
const nameOf = (index: number): string => must(byId.get(must(lands[index]))).name
const COLOUR: Record<string, string> = {
  W: 'white',
  U: 'blue',
  B: 'black',
  R: 'red',
  G: 'green',
  C: 'colorless',
}

/** A source list: `[land, 'GU']` is land number `land` with a green and then a blue ability. */
type Sources = [number, string][]
const sourcesOf = (given: Sources) =>
  given.flatMap(([index, letters]) =>
    [...letters].map((letter, ability) => source(must(lands[index]), letter, ability)),
  )
const payOptions = (text: string, given: Sources, over: Partial<PayOptions> = {}): PayOptions => ({
  spell: evangel.id,
  cost: cost(text),
  pool: poolOf(''),
  sources: sourcesOf(given),
  auto_available: true,
  can_cancel: true,
  ...over,
})
/** A payment: floating mana, and `[land, ability]` for each tap. */
const payment = (floating: string[], taps: [number, number][]): Payment => ({
  pool: floating.map(manaOfLetter),
  taps: taps.map(([index, ability]) => ({ object: must(lands[index]), ability })),
})

/** The table of seat 0 facing the payment `given`, with an `answer` spy that accepts everything. */
function renderPay(given: PayOptions) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state) => ({ ...state, pending: { id: 4, decision: { player: 0, kind: { PayMana: given } } } }),
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const land = (index: number): HTMLElement => card(must(lands[index]))
  const marks = (): Map<number, string> =>
    new Map(
      [...table.container.querySelectorAll<HTMLElement>('[data-highlight]')].map((element) => [
        Number(element.getAttribute('data-object-id')),
        must(element.getAttribute('data-highlight')),
      ]),
    )
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm payment' })
  const text = (): string =>
    must(table.container.querySelector('[data-slot="prompt-text"]')).textContent ?? ''
  const chips = (): string[] =>
    [...table.container.querySelectorAll('[data-slot="pay-chip"]')].map(
      (chip) => chip.getAttribute('aria-label') ?? '',
    )
  /** One step: `3` clicks land 3, `3/U` then picks blue in the chooser, `pool:G` spends floating green. */
  const step = async (what: string): Promise<void> => {
    if (what.startsWith('pool:')) {
      const name = COLOUR[what.slice(5)]
      await table.user.click(
        screen.getByRole('button', { name: new RegExp(`^Spend ${name} from your pool`) }),
      )
      return
    }
    const [index, pick] = what.split('/')
    await table.user.click(land(Number(index)))
    if (pick !== undefined) {
      await table.user.click(screen.getByRole('button', { name: `Add ${COLOUR[pick]} mana` }))
    }
  }
  const steps = async (list: readonly string[]): Promise<void> => {
    for (const what of list) await step(what)
  }
  return { ...table, answer, card, land, marks, confirm, text, chips, step, steps }
}

/** Payments made by clicks: the steps, and the Manual answer they must produce. */
const PAYMENTS: [string, string, string, Sources, string[], Payment][] = [
  [
    'generic from two lands',
    '{2}',
    '',
    [
      [0, 'G'],
      [1, 'G'],
      [2, 'U'],
    ],
    ['0', '1'],
    payment(
      [],
      [
        [0, 0],
        [1, 0],
      ],
    ),
  ],
  [
    'one coloured symbol',
    '{G}',
    '',
    [
      [0, 'G'],
      [1, 'U'],
    ],
    ['0'],
    payment([], [[0, 0]]),
  ],
  [
    'colour and generic',
    '{1}{G}',
    '',
    [
      [0, 'U'],
      [1, 'G'],
    ],
    ['0', '1'],
    payment(
      [],
      [
        [0, 0],
        [1, 0],
      ],
    ),
  ],
  [
    'the colour clicked first',
    '{1}{G}',
    '',
    [
      [0, 'G'],
      [1, 'G'],
    ],
    ['1', '0'],
    payment(
      [],
      [
        [1, 0],
        [0, 0],
      ],
    ),
  ],
  [
    'a double colour',
    '{R}{R}',
    '',
    [
      [0, 'R'],
      [1, 'R'],
      [2, 'R'],
    ],
    ['2', '0'],
    payment(
      [],
      [
        [2, 0],
        [0, 0],
      ],
    ),
  ],
  [
    'all five colours',
    '{W}{U}{B}{R}{G}',
    '',
    [
      [0, 'W'],
      [1, 'U'],
      [2, 'B'],
      [3, 'R'],
      [4, 'G'],
    ],
    ['4', '3', '2', '1', '0'],
    payment(
      [],
      [
        [4, 0],
        [3, 0],
        [2, 0],
        [1, 0],
        [0, 0],
      ],
    ),
  ],
  [
    'colourless',
    '{C}',
    '',
    [
      [0, 'C'],
      [1, 'G'],
    ],
    ['0'],
    payment([], [[0, 0]]),
  ],
  [
    'colourless and generic',
    '{1}{C}',
    '',
    [
      [0, 'C'],
      [1, 'G'],
    ],
    ['1', '0'],
    payment(
      [],
      [
        [1, 0],
        [0, 0],
      ],
    ),
  ],
  [
    'a big cost',
    '{3}{B}{B}',
    '',
    [
      [0, 'B'],
      [1, 'B'],
      [2, 'G'],
      [3, 'U'],
      [4, 'R'],
    ],
    ['0', '1', '2', '3', '4'],
    payment(
      [],
      [
        [0, 0],
        [1, 0],
        [2, 0],
        [3, 0],
        [4, 0],
      ],
    ),
  ],
  ['floating mana alone', '{G}', 'G', [[0, 'U']], ['pool:G'], payment(['G'], [])],
  [
    'floating mana and lands',
    '{2}{G}',
    'RG',
    [[0, 'U']],
    ['pool:R', 'pool:G', '0'],
    payment(['R', 'G'], [[0, 0]]),
  ],
  ['the colour from the pool', '{1}{G}', 'G', [[0, 'U']], ['0', 'pool:G'], payment(['G'], [[0, 0]])],
  [
    'a land with two kinds of mana',
    '{1}{U}',
    '',
    [
      [0, 'GU'],
      [1, 'G'],
    ],
    ['0/U', '1'],
    payment(
      [],
      [
        [0, 1],
        [1, 0],
      ],
    ),
  ],
  [
    'its other ability',
    '{1}{G}',
    '',
    [
      [0, 'GU'],
      [1, 'R'],
    ],
    ['0/G', '1'],
    payment(
      [],
      [
        [0, 0],
        [1, 0],
      ],
    ),
  ],
  ['a free spell', '', '', [[0, 'G']], [], payment([], [])],
]

describe('payments made by clicks', () => {
  it.each(PAYMENTS)('%s: %s', async (_what, text, floating, given, clicks, expected) => {
    const { answer, steps, confirm, user } = renderPay(payOptions(text, given, { pool: poolOf(floating) }))
    if (clicks.length > 0) expect(confirm()).toBeDisabled()
    await steps(clicks)
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: expected } }]])
  })

  it.each(PAYMENTS.filter(([, , , , clicks]) => clicks.length > 0))(
    '%s: Confirm stays disabled while a step is missing',
    async (_what, text, floating, given, clicks) => {
      const { steps, confirm } = renderPay(payOptions(text, given, { pool: poolOf(floating) }))
      await steps(clicks.slice(0, -1))
      expect(confirm()).toBeDisabled()
    },
  )
})

/** What is marked: the cost, sources, what was clicked, then the legal and selected source numbers. */
const MARKS: [string, string, Sources, string[], number[], number[]][] = [
  [
    'every source helps at the start',
    '{2}{G}',
    [
      [0, 'G'],
      [1, 'U'],
      [2, 'R'],
    ],
    [],
    [0, 1, 2],
    [],
  ],
  [
    'only the colour that is asked for',
    '{G}',
    [
      [0, 'G'],
      [1, 'U'],
    ],
    [],
    [0],
    [],
  ],
  [
    'a chosen source is selected, the others stay legal',
    '{2}{G}',
    [
      [0, 'G'],
      [1, 'U'],
      [2, 'R'],
    ],
    ['0'],
    [1, 2],
    [0],
  ],
  [
    'nothing is legal once the cost is covered',
    '{1}',
    [
      [0, 'G'],
      [1, 'U'],
    ],
    ['0'],
    [],
    [0],
  ],
  [
    'the last colour is kept for a source of that colour',
    '{1}{G}',
    [
      [0, 'U'],
      [1, 'U'],
      [2, 'G'],
    ],
    ['0'],
    [2],
    [0],
  ],
  [
    'a land of two kinds helps when one fits',
    '{G}',
    [
      [0, 'UG'],
      [1, 'R'],
    ],
    [],
    [0],
    [],
  ],
  [
    'a land of two kinds that cannot help',
    '{G}',
    [
      [0, 'UR'],
      [1, 'G'],
    ],
    [],
    [1],
    [],
  ],
  [
    'colourless does not pay for a colour',
    '{G}',
    [
      [0, 'C'],
      [1, 'G'],
    ],
    [],
    [1],
    [],
  ],
  ['colourless pays generic', '{1}', [[0, 'C']], [], [0], []],
  [
    'nothing helps a free spell',
    '',
    [
      [0, 'G'],
      [1, 'U'],
    ],
    [],
    [],
    [],
  ],
  [
    'a source can be chosen twice only as two permanents',
    '{2}',
    [
      [0, 'GU'],
      [1, 'G'],
    ],
    ['0/G'],
    [1],
    [0],
  ],
]

describe('what is marked', () => {
  it.each(MARKS)('%s', async (_what, text, given, clicks, legal, selected) => {
    const { marks, steps } = renderPay(payOptions(text, given))
    await steps(clicks)
    const flag = (index: number): string | undefined =>
      selected.includes(index) ? 'selected' : legal.includes(index) ? 'legal' : undefined
    const expected = new Map(OWN.map((id) => [id, 'dimmed']))
    for (const index of [...legal, ...selected]) expected.set(must(lands[index]), must(flag(index)))
    expect(marks()).toEqual(expected)
  })

  it('marks nothing of an opponent, the stack or a graveyard', () => {
    const { marks } = renderPay(payOptions('{2}', [[0, 'G']]))
    for (const object of view.objects)
      if (!OWN.includes(object.id)) expect(marks().has(object.id)).toBe(false)
  })

  it('says in each name whether a card can pay, and reads as pressed once chosen', async () => {
    const { land, steps } = renderPay(
      payOptions('{G}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    expect(land(0)).toHaveAccessibleName(/, available$/)
    expect(land(1)).toHaveAccessibleName(/, not available$/)
    await steps(['0'])
    expect(land(0)).toHaveAttribute('aria-pressed', 'true')
    expect(land(0).getAttribute('aria-label')).not.toMatch(/available/)
  })

  it('marks the cards for the whole time the decision is on screen and not after', () => {
    const { marks, update } = renderPay(payOptions('{G}', [[0, 'G']]))
    expect(marks().size).toBe(OWN.length)
    update(() => ({ pending: null }))
    expect(marks().size).toBe(0)
  })
})

describe('clicks that do nothing', () => {
  it('ignores a dimmed source and an opponent card', async () => {
    const { land, steps, chips, container, user } = renderPay(
      payOptions('{G}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    await steps(['1'])
    await user.click(
      must(container.querySelector<HTMLElement>('[data-battlefield-seat="1"] button[data-object-id]')),
    )
    expect(chips()).toEqual([])
    expect(land(1)).toHaveAttribute('data-highlight', 'dimmed')
  })

  it('ignores a source once the cost is covered', async () => {
    const { steps, chips } = renderPay(
      payOptions('{1}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    await steps(['0', '1'])
    expect(chips()).toEqual([`Remove green from ${nameOf(0)}`])
  })
})

describe('taking a choice back', () => {
  const given: Sources = [
    [0, 'G'],
    [1, 'U'],
    [2, 'R'],
  ]

  it('clicking a chosen source again takes it back', async () => {
    const { land, steps, chips, confirm } = renderPay(payOptions('{2}', given))
    await steps(['0', '1'])
    expect(confirm()).toBeEnabled()
    await steps(['0'])
    expect(chips()).toEqual([`Remove blue from ${nameOf(1)}`])
    expect(land(0)).toHaveAttribute('data-highlight', 'legal')
    expect(confirm()).toBeDisabled()
  })

  it('a chip takes back its own choice', async () => {
    const { steps, chips, user } = renderPay(payOptions('{3}', given))
    await steps(['0', '1', '2'])
    await user.click(screen.getByRole('button', { name: `Remove blue from ${nameOf(1)}` }))
    expect(chips()).toEqual([`Remove green from ${nameOf(0)}`, `Remove red from ${nameOf(2)}`])
  })

  it('Undo takes back the last choice', async () => {
    const { steps, chips, user } = renderPay(payOptions('{3}', given))
    await steps(['0', '1', '2'])
    await user.click(screen.getByRole('button', { name: 'Undo the last choice' }))
    expect(chips()).toEqual([`Remove green from ${nameOf(0)}`, `Remove blue from ${nameOf(1)}`])
  })

  it('Clear takes back everything, and Undo and Clear go with it', async () => {
    const { steps, chips, user, text } = renderPay(payOptions('{3}', given, { pool: poolOf('G') }))
    await steps(['0', '1', 'pool:G'])
    await user.click(screen.getByRole('button', { name: 'Clear the payment' }))
    expect(chips()).toEqual([])
    expect(screen.queryByRole('button', { name: 'Undo the last choice' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Clear the payment' })).toBeNull()
    expect(text()).toMatch(/^Cleared the payment\. Pay 3 generic/)
  })

  it('floating mana comes back to the pool when its chip is removed', async () => {
    const { steps, chips, user } = renderPay(payOptions('{2}', [[0, 'G']], { pool: poolOf('RR') }))
    await steps(['pool:R', 'pool:R'])
    expect(screen.getByRole('button', { name: 'Spend red from your pool, 0 left' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    await user.click(screen.getAllByRole('button', { name: 'Remove red from your pool' })[0] as HTMLElement)
    expect(chips()).toEqual(['Remove red from your pool'])
    expect(screen.getByRole('button', { name: 'Spend red from your pool, 1 left' })).toHaveAttribute(
      'aria-disabled',
      'false',
    )
  })
})

describe('floating mana', () => {
  it('shows each kind that is held with how many are left', () => {
    renderPay(payOptions('{3}', [], { pool: poolOf('RRG') }))
    expect(screen.getByRole('button', { name: 'Spend red from your pool, 2 left' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Spend green from your pool, 1 left' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^Spend blue/ })).toBeNull()
  })

  it('says none when the pool is empty', () => {
    renderPay(payOptions('{3}', [[0, 'G']]))
    expect(within(screen.getByRole('group', { name: 'Floating mana' })).getByText('none')).toBeInTheDocument()
  })

  it('does not spend mana the cost cannot use', async () => {
    const { steps, chips } = renderPay(payOptions('{G}', [], { pool: poolOf('R') }))
    const red = screen.getByRole('button', { name: 'Spend red from your pool, 1 left' })
    expect(red).toHaveAttribute('aria-disabled', 'true')
    await steps(['pool:R'])
    expect(chips()).toEqual([])
  })
})

describe('a land with several kinds of mana', () => {
  const given: Sources = [
    [0, 'GU'],
    [1, 'G'],
  ]
  const group = (): HTMLElement => screen.getByRole('group', { name: `Mana from ${nameOf(0)}` })

  it('asks which, with only the kinds that still fit, and the choice adds that ability', async () => {
    const { land, user, answer, chips } = renderPay(
      payOptions('{1}{G}', [
        [0, 'GUR'],
        [1, 'G'],
      ]),
    )
    await user.click(land(0))
    expect(answer).not.toHaveBeenCalled()
    expect(
      within(group())
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label') ?? button.textContent),
    ).toEqual(['Add green mana', 'Add blue mana', 'Add red mana', 'Cancel'])
    await user.click(screen.getByRole('button', { name: 'Add red mana' }))
    expect(chips()).toEqual([`Remove red from ${nameOf(0)}`])
  })

  it('leaves out a kind that would be wasted', async () => {
    const { land, user, chips } = renderPay(
      payOptions('{1}{G}', [
        [0, 'GUR'],
        [1, 'U'],
      ]),
    )
    await user.click(land(1))
    await user.click(land(0))
    // Another blue or any red would leave the green unpaid, so green is the only choice and needs no question.
    expect(screen.queryByRole('group', { name: `Mana from ${nameOf(0)}` })).toBeNull()
    expect(chips()).toEqual([`Remove blue from ${nameOf(1)}`, `Remove green from ${nameOf(0)}`])
  })

  it('marks the land selected while asking', async () => {
    const { land, user, marks } = renderPay(payOptions('{2}', given))
    await user.click(land(0))
    expect(marks().get(must(lands[0]))).toBe('selected')
    expect(marks().get(must(lands[1]))).toBe('legal')
  })

  it('does not ask when two abilities make the same mana, and adds the first', async () => {
    const { land, user, chips } = renderPay(payOptions('{2}', [[0, 'RR']]))
    await user.click(land(0))
    expect(screen.queryByRole('group', { name: `Mana from ${nameOf(0)}` })).toBeNull()
    expect(chips()).toEqual([`Remove red from ${nameOf(0)}`])
  })

  it('adds the single kind that fits without asking, with the ability that makes it', async () => {
    const { land, user, chips, confirm, answer } = renderPay(payOptions('{G}', [[0, 'UG']]))
    await user.click(land(0))
    expect(chips()).toEqual([`Remove green from ${nameOf(0)}`])
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: payment([], [[0, 1]]) } }]])
  })

  it('answers with the chosen ability, not the first', async () => {
    const { land, user, confirm, answer } = renderPay(payOptions('{1}', [[0, 'GU']]))
    await user.click(land(0))
    await user.click(screen.getByRole('button', { name: 'Add blue mana' }))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: payment([], [[0, 1]]) } }]])
  })

  it('moves focus into the choice, and back to the land after it', async () => {
    const { land, user } = renderPay(payOptions('{2}', given))
    await user.click(land(0))
    expect(screen.getByRole('button', { name: 'Add green mana' })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Add blue mana' }))
    expect(land(0)).toHaveFocus()
  })

  it.each([
    ['Escape', async ({ user }: Pick<ReturnType<typeof renderPay>, 'user'>) => user.keyboard('{Escape}')],
    [
      'the Cancel button in the choice',
      async ({ user }: Pick<ReturnType<typeof renderPay>, 'user'>) =>
        user.click(within(group()).getByRole('button', { name: 'Cancel' })),
    ],
  ])('is closed by %s, leaving focus on the land and the payment alone', async (_how, close) => {
    const table = renderPay(payOptions('{2}', given))
    await table.user.click(table.land(0))
    await close(table)
    expect(screen.queryByRole('group', { name: `Mana from ${nameOf(0)}` })).toBeNull()
    expect(table.land(0)).toHaveAttribute('data-highlight', 'legal')
    expect(table.land(0)).toHaveFocus()
    expect(table.chips()).toEqual([])
    expect(table.answer).not.toHaveBeenCalled()
  })

  it('is closed by clicking the land again', async () => {
    const { land, user } = renderPay(payOptions('{2}', given))
    await user.click(land(0))
    await user.click(land(0))
    expect(screen.queryByRole('group', { name: `Mana from ${nameOf(0)}` })).toBeNull()
  })

  it('moves to another land with several kinds', async () => {
    const { land, user, marks } = renderPay(
      payOptions('{2}', [
        [0, 'GU'],
        [1, 'RB'],
      ]),
    )
    await user.click(land(0))
    await user.click(land(1))
    expect(marks().get(must(lands[0]))).toBe('legal')
    expect(marks().get(must(lands[1]))).toBe('selected')
    expect(screen.getByRole('group', { name: `Mana from ${nameOf(1)}` })).toBeInTheDocument()
  })

  it('closes when another source is added meanwhile', async () => {
    const { land, user } = renderPay(
      payOptions(
        '{2}',
        [
          [0, 'GU'],
          [1, 'R'],
        ],
        { pool: poolOf('G') },
      ),
    )
    await user.click(land(0))
    await user.click(screen.getByRole('button', { name: 'Spend green from your pool, 1 left' }))
    expect(screen.queryByRole('group', { name: `Mana from ${nameOf(0)}` })).toBeNull()
  })
})

describe('Pay automatically, Cancel and Confirm', () => {
  it('Pay automatically answers Auto while nothing is chosen', async () => {
    const { answer, user } = renderPay(
      payOptions('{2}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    expect(screen.queryByRole('button', { name: 'Pay the rest for me' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Pay automatically' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: 'Auto' }]])
  })

  it('has no Pay automatically when the game offers none', () => {
    renderPay(payOptions('{U}{U}', [[0, 'G']], { auto_available: false }))
    expect(screen.queryByRole('button', { name: 'Pay automatically' })).toBeNull()
  })

  it.each([
    ['one land', ['0'], payment([], [[0, 0]])],
    [
      'two lands in the order chosen',
      ['1', '0'],
      payment(
        [],
        [
          [1, 0],
          [0, 0],
        ],
      ),
    ],
    ['a land and floating mana', ['0', 'pool:R'], payment(['R'], [[0, 0]])],
    ['floating mana alone', ['pool:R'], payment(['R'], [])],
  ])('Pay the rest for me keeps %s and answers Complete', async (_what, picks, expected) => {
    const { answer, user, steps } = renderPay(
      payOptions(
        '{4}{G}',
        [
          [0, 'G'],
          [1, 'U'],
          [2, 'B'],
          [3, 'W'],
        ],
        { pool: poolOf('R') },
      ),
    )
    await steps(picks)
    expect(screen.queryByRole('button', { name: 'Pay automatically' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Pay the rest for me' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: { Complete: expected } }]])
  })

  it.each([
    [
      'the game offers no automatic payment',
      payOptions('{G}{G}', [[0, 'G']], { auto_available: false }),
      ['0'],
    ],
    ['the payment already covers the cost', payOptions('{G}', [[0, 'G']]), ['0']],
    [
      'nothing has been chosen',
      payOptions('{2}', [
        [0, 'G'],
        [1, 'U'],
      ]),
      [],
    ],
  ])('has no Pay the rest for me when %s', async (_what, given, picks) => {
    const { steps } = renderPay(given)
    await steps(picks)
    expect(screen.queryByRole('button', { name: 'Pay the rest for me' })).toBeNull()
  })

  it('goes back to Pay automatically when the picks are taken back', async () => {
    const { user, steps, chips } = renderPay(
      payOptions('{2}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    await steps(['0'])
    await user.click(screen.getByRole('button', { name: /^Remove green/ }))
    expect(chips()).toEqual([])
    expect(screen.queryByRole('button', { name: 'Pay the rest for me' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Pay automatically' })).toBeInTheDocument()
  })

  it('has no Cancel when the options do not allow backing out, and says so in the sentence', () => {
    const { text } = renderPay(payOptions('{U}{U}', [[0, 'G']], { auto_available: false, can_cancel: false }))
    expect(screen.queryByRole('button', { name: 'Cancel casting' })).toBeNull()
    expect(text()).toBe('You cannot pay 2 blue for Selesnya Evangel with the mana you have.')
  })

  it.each([
    ['with sources', payOptions('{2}', [[0, 'G']])],
    ['with no sources and no automatic payment', payOptions('{2}', [], { auto_available: false })],
    ['for a free spell', payOptions('', [])],
  ])('Cancel answers Cancel %s', async (_what, given) => {
    const { answer, user } = renderPay(given)
    await user.click(screen.getByRole('button', { name: 'Cancel casting' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: 'Cancel' }]])
  })

  it('Cancel works with a payment half made', async () => {
    const { answer, user, steps } = renderPay(
      payOptions('{2}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    await steps(['0'])
    await user.click(screen.getByRole('button', { name: 'Cancel casting' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: 'Cancel' }]])
  })

  it('Confirm cannot be pressed before the payment covers the cost', async () => {
    const { answer, user, confirm, steps } = renderPay(
      payOptions('{2}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    await steps(['0'])
    expect(confirm()).toBeDisabled()
    await user.click(confirm())
    expect(answer).not.toHaveBeenCalled()
  })

  it.each([
    ['Pay automatically', [], { PayMana: 'Auto' }],
    ['Pay the rest for me', ['0'], { PayMana: { Complete: payment([], [[0, 0]]) } }],
    ['Cancel casting', [], { PayMana: 'Cancel' }],
    [
      'Confirm payment',
      ['0', '1'],
      {
        PayMana: {
          Manual: payment(
            [],
            [
              [0, 0],
              [1, 0],
            ],
          ),
        },
      },
    ],
  ])('a double click on %s sends one answer', async (name, picks, expected) => {
    const { answer, user, steps } = renderPay(
      payOptions('{G}{U}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    await steps(picks)
    await user.dblClick(screen.getByRole('button', { name }))
    expect(answer.mock.calls).toEqual([[expected]])
  })
})

describe('the keyboard', () => {
  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('%s on a legal source adds it, and on a chosen one takes it back', async (_key, keys) => {
    const { land, user, chips } = renderPay(
      payOptions('{2}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    act(() => land(0).focus())
    await user.keyboard(keys)
    expect(chips()).toEqual([`Remove green from ${nameOf(0)}`])
    await user.keyboard(keys)
    expect(chips()).toEqual([])
  })

  it('does nothing on a dimmed source', async () => {
    const { land, user, chips } = renderPay(
      payOptions('{G}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    act(() => land(1).focus())
    await user.keyboard('{Enter} ')
    expect(chips()).toEqual([])
  })

  it('pays from the keyboard alone: sources, the choice, then Confirm', async () => {
    const { land, user, confirm, answer } = renderPay(
      payOptions('{1}{U}', [
        [0, 'GU'],
        [1, 'G'],
      ]),
    )
    act(() => land(0).focus())
    await user.keyboard('{Enter}')
    await user.keyboard('{Tab}{Enter}')
    expect(land(0)).toHaveFocus()
    act(() => land(1).focus())
    await user.keyboard('{Enter}')
    act(() => confirm().focus())
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([
      [
        {
          PayMana: {
            Manual: payment(
              [],
              [
                [0, 1],
                [1, 0],
              ],
            ),
          },
        },
      ],
    ])
  })

  it('takes a chip back with Enter and keeps the keyboard on the next one', async () => {
    const { steps, user } = renderPay(
      payOptions('{3}', [
        [0, 'G'],
        [1, 'U'],
        [2, 'R'],
      ]),
    )
    await steps(['0', '1', '2'])
    const first = screen.getByRole('button', { name: `Remove green from ${nameOf(0)}` })
    act(() => first.focus())
    await user.keyboard('{Enter}')
    expect(screen.getByRole('button', { name: `Remove blue from ${nameOf(1)}` })).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(screen.getByRole('button', { name: `Remove red from ${nameOf(2)}` })).toHaveFocus()
    await user.keyboard('{Enter}')
    // With nothing left to take back, the keyboard stays inside the prompt.
    expect(document.activeElement?.closest('[data-slot="decision-line"]')).not.toBeNull()
  })

  it('keeps the keyboard in the prompt after Clear', async () => {
    const { steps, user } = renderPay(
      payOptions('{2}', [
        [0, 'G'],
        [1, 'U'],
      ]),
    )
    await steps(['0', '1'])
    await user.click(screen.getByRole('button', { name: 'Clear the payment' }))
    expect(document.activeElement?.closest('[data-slot="decision-line"]')).not.toBeNull()
  })
})

describe('a new decision', () => {
  it('starts with no payment, no choice open and the new options', async () => {
    const { update, steps, chips, land, marks, confirm } = renderPay(
      payOptions('{2}', [
        [0, 'GU'],
        [1, 'U'],
      ]),
    )
    await steps(['1'])
    await steps(['0'])
    expect(screen.getByRole('group', { name: `Mana from ${nameOf(0)}` })).toBeInTheDocument()
    update(() => ({
      pending: {
        id: 5,
        decision: {
          player: 0,
          kind: {
            PayMana: payOptions('{G}', [
              [0, 'G'],
              [2, 'R'],
            ]),
          },
        },
      },
    }))
    expect(chips()).toEqual([])
    expect(screen.queryByRole('group', { name: `Mana from ${nameOf(0)}` })).toBeNull()
    expect(confirm()).toBeDisabled()
    expect(land(0)).toHaveAttribute('data-highlight', 'legal')
    expect(land(1)).toHaveAttribute('data-highlight', 'dimmed')
    expect(land(2)).toHaveAttribute('data-highlight', 'dimmed')
    expect(marks().size).toBe(OWN.length)
  })
})

describe('what the player is told', () => {
  it('shows the cost, the owed mana and the payment as they change', async () => {
    const { steps, container } = renderPay(
      payOptions('{2}{G}', [
        [0, 'G'],
        [1, 'U'],
        [2, 'R'],
      ]),
    )
    const cost = (): string | null =>
      container
        .querySelector('[data-slot="payment"] [role="img"][aria-label^="Mana cost"]')
        ?.getAttribute('aria-label') ?? null
    const still = (): string =>
      container.querySelector('[aria-label^="Mana still owed"]')?.getAttribute('aria-label') ??
      within(screen.getByRole('group', { name: 'Still owed' })).getByText(/covered/).textContent ??
      ''
    expect(cost()).toBe('Mana cost: 2 generic, green')
    expect(still()).toBe('Mana still owed: 2 generic, green')
    await steps(['0'])
    expect(still()).toBe('Mana still owed: 2 generic')
    await steps(['1', '2'])
    expect(still()).toBe('nothing, the cost is covered')
    expect(cost()).toBe('Mana cost: 2 generic, green')
  })

  it('says the same in the one live sentence', async () => {
    const { steps, text } = renderPay(
      payOptions('{2}{G}', [
        [0, 'G'],
        [1, 'U'],
        [2, 'R'],
      ]),
    )
    expect(text()).toBe(
      'Pay 2 generic and 1 green for Selesnya Evangel. Choose a land or other mana source to use its mana, or pay automatically.',
    )
    await steps(['1'])
    expect(text()).toBe(
      `Added blue from ${nameOf(1)}. Still to pay: 1 generic and 1 green. Choose more, or pay the rest for me.`,
    )
    await steps(['0', '2'])
    expect(text()).toBe(`Added red from ${nameOf(2)}. The cost is covered. Confirm to cast Selesnya Evangel.`)
    await steps(['2'])
    expect(text()).toBe(
      `Removed red from ${nameOf(2)}. Still to pay: 1 generic. Choose more, or pay the rest for me.`,
    )
  })

  it('says when nothing left can pay', async () => {
    const { steps, text, land } = renderPay(
      payOptions(
        '{U}{U}',
        [
          [0, 'U'],
          [1, 'G'],
          [2, 'R'],
        ],
        { auto_available: false },
      ),
    )
    await steps(['0', '1'])
    expect(land(1)).toHaveAttribute('data-highlight', 'dimmed')
    expect(text()).toBe(
      `Added blue from ${nameOf(0)}. Still to pay: 1 blue. Nothing left can pay it; remove a choice or cancel.`,
    )
  })

  it('says so at once when the spell cannot be paid for', () => {
    const { text, confirm } = renderPay(
      payOptions('{U}{U}', [[0, 'G']], { auto_available: false, pool: poolOf('') }),
    )
    expect(confirm()).toBeDisabled()
    expect(text()).toBe(
      'You cannot pay 2 blue for Selesnya Evangel with the mana you have. Cancel to go back.',
    )
  })

  it('names the spell and says a free one costs nothing', () => {
    const { text } = renderPay(payOptions('', []))
    expect(text()).toBe('Selesnya Evangel costs nothing. Confirm to cast it.')
  })

  it('is one polite live region under the name of the decision', () => {
    const { container } = renderPay(payOptions('{G}', [[0, 'G']]))
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(within(line).getByText('Your decision: Pay mana')).toBeInTheDocument()
    expect(line.querySelectorAll('[aria-live]')).toHaveLength(1)
    expect(must(line.querySelector('[aria-live]'))).toHaveAttribute('aria-live', 'polite')
    expect(must(line.querySelector('[aria-live]'))).toHaveAttribute('aria-atomic', 'true')
  })
})

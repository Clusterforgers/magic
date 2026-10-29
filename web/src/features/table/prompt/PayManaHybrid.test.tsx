/** Goal: for hybrid, monocolored hybrid and Phyrexian costs the PayMana prompt marks the sources that can pay, lets the player pay 2 life for each Phyrexian symbol within the life total, shows the life that leaves, keeps the sentence, Pay automatically and Pay the rest for me working, and sends the payment with its life as one answer; a plain cost shows no life at all. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId, PayOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { cost, poolOf, source } from '@/test-utils/mana'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'
import { canPayLife } from '../manaSymbols'

const fixture = buildFixture({ seats: 2 })
const view = fixture.view
const lands = view.objects
  .filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === 0 &&
      object.types.includes('Land') &&
      object.permanent?.tapped === false,
  )
  .map((object) => object.id)
const spell = must(view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === 0))

/** Sources by land number: `'WU'` is land 0 making white and land 1 making blue. */
const sourcesOf = (letters: string) => [...letters].map((letter, index) => source(must(lands[index]), letter))

/** The most life a payment may take: 2 for each Phyrexian symbol, within the life total (rounded down to pairs). */
const maxLife = (text: string, life: number): number =>
  Math.min(2 * cost(text).filter(canPayLife).length, Math.max(0, life - (life % 2)))

const payOptions = (text: string, letters: string, over: Partial<PayOptions> = {}): PayOptions => {
  const life = over.life ?? 20
  return {
    spell: spell.id,
    cost: cost(text),
    pool: poolOf(''),
    sources: sourcesOf(letters),
    auto_available: true,
    can_cancel: true,
    life,
    max_life: maxLife(text, life),
    ...over,
  }
}

function renderPay(given: PayOptions) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    session: { answer },
    state: (state) => ({ ...state, pending: { id: 4, decision: { player: 0, kind: { PayMana: given } } } }),
  })
  const land = (index: number): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${must(lands[index])}"]`))
  const marked = (): number[] =>
    lands.flatMap((id, index) =>
      table.container.querySelector(`button[data-object-id="${id}"]`)?.getAttribute('data-highlight') ===
      'legal'
        ? [index]
        : [],
    )
  const text = (): string =>
    must(table.container.querySelector('[data-slot="prompt-text"]')).textContent ?? ''
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm payment' })
  const lifeSwitches = (): HTMLElement[] => [
    ...table.container.querySelectorAll<HTMLElement>('[data-slot="pay-life"]'),
  ]
  const lifeRow = (): HTMLElement => screen.getByRole('group', { name: 'Life' })
  return { ...table, answer, land, marked, text, confirm, lifeSwitches, lifeRow }
}

const tap = (index: number, ability = 0) => ({ object: must(lands[index] as ObjectId), ability })

describe('a hybrid cost', () => {
  it.each([
    ['the first colour', 0],
    ['the other colour', 1],
  ])('is paid by %s', async (_what, index) => {
    const { answer, land, marked, confirm, user } = renderPay(payOptions('{W/U}', 'WUG'))
    expect(marked()).toEqual([0, 1])
    await user.click(land(index))
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [tap(index)], life: 0 } } }]])
  })

  it('is not paid by a third colour, and says it can take either', () => {
    const { marked, text } = renderPay(payOptions('{W/U}', 'GRB'))
    expect(marked()).toEqual([])
    expect(text()).toBe(
      'You cannot pay 1 white or blue for Selesnya Evangel with the mana you have. Cancel to go back.',
    )
  })

  it('keeps a hybrid symbol hybrid in what is still owed until mana meets it', async () => {
    const { land, user, container } = renderPay(payOptions('{R/W}{R/W}', 'RWR'))
    const owed = (): string | null =>
      container.querySelector('[aria-label^="Mana still owed"]')?.getAttribute('aria-label') ?? null
    expect(owed()).toBe('Mana still owed: red or white mana, red or white mana')
    await user.click(land(0))
    expect(owed()).toBe('Mana still owed: red or white mana')
    await user.click(land(1))
    expect(owed()).toBeNull()
  })

  it('draws the cost with a split symbol and names it in words', () => {
    const { container } = renderPay(payOptions('{2}{R/W}', 'RW'))
    const cost = container.querySelector('[data-slot="payment"] [role="img"][aria-label^="Mana cost"]')
    expect(cost).toHaveAttribute('aria-label', 'Mana cost: 2 generic, red or white mana')
    expect(container.querySelector('[data-slot="payment"] [data-pip="R/W"]')).not.toBeNull()
  })
})

describe('a monocolored hybrid cost', () => {
  it('is paid by its colour alone', async () => {
    const { land, confirm, user, answer } = renderPay(payOptions('{2/W}', 'GGW'))
    await user.click(land(2))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [tap(2)], life: 0 } } }]])
  })

  it('is paid by two of anything, and the colour then no longer fits', async () => {
    const { land, marked, confirm, user, answer, text } = renderPay(payOptions('{2/W}', 'GGW'))
    expect(marked()).toEqual([0, 1, 2])
    await user.click(land(0))
    expect(text()).toMatch(/Still to pay: 1 generic\./)
    expect(confirm()).toBeDisabled()
    await user.click(land(1))
    expect(confirm()).toBeEnabled()
    expect(marked()).toEqual([])
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([
      [{ PayMana: { Manual: { pool: [], taps: [tap(0), tap(1)], life: 0 } } }],
    ])
  })

  it('takes the colour as the second of two generic mana too', async () => {
    const { land, confirm, user } = renderPay(payOptions('{2/W}', 'GW'))
    await user.click(land(0))
    await user.click(land(1))
    expect(confirm()).toBeEnabled()
  })
})

describe('a Phyrexian cost', () => {
  it('shows the life row with a switch for the symbol and the life total', () => {
    const { lifeSwitches, lifeRow } = renderPay(payOptions('{W/P}', 'G'))
    expect(within(lifeRow()).getByText('You have 20 life.')).toBeInTheDocument()
    const [only, ...others] = lifeSwitches()
    expect(others).toEqual([])
    expect(only).toHaveAccessibleName('Pay 2 life instead of white mana')
    expect(only).toHaveAttribute('aria-pressed', 'false')
    expect(only).toHaveAttribute('aria-disabled', 'false')
  })

  it('is paid with 2 life, and the answer says so', async () => {
    const { lifeSwitches, confirm, user, answer, text, lifeRow } = renderPay(payOptions('{W/P}', 'G'))
    expect(confirm()).toBeDisabled()
    await user.click(must(lifeSwitches()[0]))
    expect(must(lifeSwitches()[0])).toHaveAttribute('aria-pressed', 'true')
    expect(within(lifeRow()).getByText('You have 20 life, 18 after paying 2.')).toBeInTheDocument()
    expect(text()).toBe(
      'Paying 2 life instead of white mana. The cost is covered, paying 2 life (18 left). Confirm to cast Selesnya Evangel.',
    )
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [], life: 2 } } }]])
  })

  it('is paid with its colour instead, and no life is paid', async () => {
    const { land, marked, confirm, user, answer } = renderPay(payOptions('{W/P}', 'WG'))
    expect(marked()).toEqual([0])
    await user.click(land(0))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [tap(0)], life: 0 } } }]])
  })

  it('pays the generic part with mana and the Phyrexian part with life', async () => {
    const { land, lifeSwitches, confirm, user, answer, text } = renderPay(payOptions('{1}{W/P}', 'G'))
    await user.click(must(lifeSwitches()[0]))
    expect(text()).toMatch(/Still to pay: 1 generic\./)
    await user.click(land(0))
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [tap(0)], life: 2 } } }]])
  })

  it('takes the life back with the same switch', async () => {
    const { lifeSwitches, confirm, user, text } = renderPay(payOptions('{W/P}', 'G'))
    await user.click(must(lifeSwitches()[0]))
    await user.click(must(lifeSwitches()[0]))
    expect(must(lifeSwitches()[0])).toHaveAttribute('aria-pressed', 'false')
    expect(confirm()).toBeDisabled()
    expect(text()).toMatch(/^No longer paying 2 life instead of white mana\./)
  })

  it('offers life in the sentence beside the lands', () => {
    const { text } = renderPay(payOptions('{1}{W/P}', 'G'))
    expect(text()).toBe(
      'Pay 1 generic and 1 white or 2 life for Selesnya Evangel. Choose a land or other mana source to use its mana, pay 2 life for a Phyrexian symbol, or pay automatically.',
    )
  })

  it.each([
    ['not enough life', 1, 0],
    ['no life at all', 0, 0],
  ])('refuses the switch with %s', async (_what, life, max) => {
    const { lifeSwitches, confirm, user, answer, lifeRow } = renderPay(
      payOptions('{W/P}', 'G', { life, max_life: max, auto_available: false }),
    )
    expect(within(lifeRow()).getByText(`You have ${life} life.`)).toBeInTheDocument()
    expect(must(lifeSwitches()[0])).toHaveAttribute('aria-disabled', 'true')
    await user.click(must(lifeSwitches()[0]))
    expect(must(lifeSwitches()[0])).toHaveAttribute('aria-pressed', 'false')
    expect(confirm()).toBeDisabled()
    expect(answer).not.toHaveBeenCalled()
  })

  it('may pay all the life there is, and says what is left', async () => {
    const { lifeSwitches, confirm, user, answer, lifeRow } = renderPay(
      payOptions('{W/P}', '', { life: 2, auto_available: false }),
    )
    await user.click(must(lifeSwitches()[0]))
    expect(within(lifeRow()).getByText('You have 2 life, 0 after paying 2.')).toBeInTheDocument()
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [], life: 2 } } }]])
  })

  it('has no switch past the most life the options allow, and gives it back when one is taken back', async () => {
    // {B/P}{B/P} at a life total of 3: 2 life can be paid, for one symbol but not both.
    const { lifeSwitches, user } = renderPay(payOptions('{B/P}{B/P}', 'G', { life: 3 }))
    const names = lifeSwitches().map((button) => button.getAttribute('aria-label'))
    expect(names).toEqual([
      'Pay 2 life instead of black mana, 1 of 2',
      'Pay 2 life instead of black mana, 2 of 2',
    ])
    await user.click(must(lifeSwitches()[0]))
    expect(lifeSwitches().map((button) => button.getAttribute('aria-disabled'))).toEqual(['false', 'true'])
    await user.click(must(lifeSwitches()[1]))
    expect(lifeSwitches().map((button) => button.getAttribute('aria-pressed'))).toEqual(['true', 'false'])
    await user.click(must(lifeSwitches()[0]))
    expect(lifeSwitches().map((button) => button.getAttribute('aria-disabled'))).toEqual(['false', 'false'])
  })

  it('pays for both symbols with 4 life when there is room', async () => {
    const { lifeSwitches, land, confirm, user, answer } = renderPay(payOptions('{1}{B/P}{B/P}', 'G'))
    await user.click(must(lifeSwitches()[0]))
    await user.click(must(lifeSwitches()[1]))
    await user.click(land(0))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [tap(0)], life: 4 } } }]])
  })

  it('refuses life that the mana already chosen has no place beside, until that mana is taken back', async () => {
    // {1}{W/P}: a Forest and a Plains chosen cover the cost, so life for the symbol would leave one of them over.
    const { land, lifeSwitches, user } = renderPay(payOptions('{1}{W/P}', 'GW'))
    await user.click(land(0))
    await user.click(land(1))
    expect(must(lifeSwitches()[0])).toHaveAttribute('aria-disabled', 'true')
    await user.click(land(1))
    expect(must(lifeSwitches()[0])).toHaveAttribute('aria-disabled', 'false')
  })

  it('shows a Phyrexian cost with the phi mark and says it in words', () => {
    const { container } = renderPay(payOptions('{2}{W/P}{G/U/P}', 'G'))
    const label = container
      .querySelector('[data-slot="payment"] [role="img"][aria-label^="Mana cost"]')
      ?.getAttribute('aria-label')
    expect(label).toBe('Mana cost: 2 generic, white mana or 2 life, green or blue mana or 2 life')
    expect(container.querySelector('[data-slot="payment"] [data-pip="W/P"]')?.textContent).toBe('ϕ')
    expect(container.querySelector('[data-slot="payment"] [data-pip="G/U/P"]')?.textContent).toBe('ϕ')
  })
})

describe('Pay automatically and Pay the rest for me with life', () => {
  it('Pay automatically is offered while nothing is chosen, and answers Auto', async () => {
    const { user, answer } = renderPay(payOptions('{1}{W/P}', 'G'))
    await user.click(screen.getByRole('button', { name: 'Pay automatically' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: 'Auto' }]])
  })

  it('goes away once life is chosen, and Pay the rest for me keeps the life', async () => {
    const { lifeSwitches, user, answer } = renderPay(payOptions('{1}{B/P}{B/P}', 'G'))
    await user.click(must(lifeSwitches()[0]))
    expect(screen.queryByRole('button', { name: 'Pay automatically' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Pay the rest for me' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: { Complete: { pool: [], taps: [], life: 2 } } }]])
  })

  it('Pay the rest for me keeps the lands and the life together', async () => {
    const { land, lifeSwitches, user, answer } = renderPay(payOptions('{1}{B/P}{B/P}', 'GB'))
    await user.click(land(1))
    await user.click(must(lifeSwitches()[1]))
    await user.click(screen.getByRole('button', { name: 'Pay the rest for me' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: { Complete: { pool: [], taps: [tap(1)], life: 2 } } }]])
  })

  it('offers no Pay automatically when it would take the last life, but still lets life be paid by hand', async () => {
    const { lifeSwitches, user, answer, text } = renderPay(
      payOptions('{W/P}', '', { life: 2, auto_available: false }),
    )
    expect(screen.getByRole('button', { name: 'Pay automatically' })).toBeDisabled()
    expect(text()).toBe(
      'Pay 1 white or 2 life for Selesnya Evangel. Choose a land or other mana source to use its mana, pay 2 life for a Phyrexian symbol.',
    )
    await user.click(must(lifeSwitches()[0]))
    await user.click(screen.getByRole('button', { name: 'Confirm payment' }))
    expect(answer).toHaveBeenCalledTimes(1)
  })

  it('Clear takes the life back as well as the lands', async () => {
    const { land, lifeSwitches, user, container } = renderPay(payOptions('{1}{W/P}', 'G'))
    await user.click(land(0))
    await user.click(must(lifeSwitches()[0]))
    await user.click(screen.getByRole('button', { name: 'Clear the payment' }))
    expect(must(lifeSwitches()[0])).toHaveAttribute('aria-pressed', 'false')
    expect(container.querySelectorAll('[data-slot="pay-chip"]')).toHaveLength(0)
  })
})

describe('colorless hybrid and the colorless Phyrexian symbol', () => {
  it('pays {C/W} with colorless or white, not with another colour', async () => {
    const { marked, land, confirm, user, answer } = renderPay(payOptions('{C/W}', 'CWG'))
    expect(marked()).toEqual([0, 1])
    await user.click(land(0))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [tap(0)], life: 0 } } }]])
  })

  it('pays {C/P} with colorless or 2 life', async () => {
    const { marked, lifeSwitches, user, confirm, answer } = renderPay(payOptions('{C/P}', 'CG'))
    expect(marked()).toEqual([0])
    expect(must(lifeSwitches()[0])).toHaveAccessibleName('Pay 2 life instead of colorless mana')
    await user.click(must(lifeSwitches()[0]))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ PayMana: { Manual: { pool: [], taps: [], life: 2 } } }]])
  })
})

describe('a plain cost', () => {
  it('shows no life row, no switch and no mention of life', () => {
    const { container, text } = renderPay(payOptions('{1}{G}', 'GU'))
    expect(container.querySelector('[data-slot="pay-life"]')).toBeNull()
    expect(screen.queryByRole('group', { name: 'Life' })).toBeNull()
    expect(text()).not.toMatch(/life/i)
  })
})

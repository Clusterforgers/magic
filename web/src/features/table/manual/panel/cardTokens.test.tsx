// Goal: the Tokens row offers the tokens the seat's cards make (`TableView.tokens`) between the common tokens and "Custom creature…", shows the chosen one's abilities in its own words and the card that makes it, and sends `ManualToken.Card` with the card and place; a seat whose cards make none sees no such group, and a choice the list no longer has is refused before Apply.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualAction, TokenOfferView } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { CommandsPanel } from './CommandsPanel'
import { cardChoice, START_TOKEN, tokenPreview } from './customToken'

const SCION: TokenOfferView = {
  card: 'Abstruse Interference',
  index: 0,
  name: 'Eldrazi Scion',
  colors: [],
  types: ['Creature'],
  subtypes: ['Eldrazi', 'Scion'],
  power: 1,
  toughness: 1,
  rules: ['Sacrifice this token: Add {C}.'],
}

const CLUE: TokenOfferView = {
  card: 'Angelic Sleuth',
  index: 0,
  name: 'Clue',
  types: ['Artifact'],
  subtypes: ['Clue'],
  rules: ['{2}, Sacrifice this token: Draw a card.'],
}

const OFFERS = [SCION, CLUE]

describe('the preview of a token of the seat’s cards', () => {
  it.each<[string, number, number, string, string]>([
    [
      'a creature',
      0,
      2,
      '2 Eldrazi Scion 1/1 creature tokens with its ability enter under your control (from Abstruse Interference)',
      'Create 2 Eldrazi Scion 1/1 tokens',
    ],
    [
      'a token without power',
      1,
      1,
      '1 Clue artifact token with its ability enters under your control (from Angelic Sleuth)',
      'Create 1 Clue token',
    ],
  ])('words %s and sends the card and place', (_name, at, amount, preview, apply) => {
    const offer = OFFERS[at] as TokenOfferView
    expect(tokenPreview({ ...START_TOKEN, token: cardChoice(at) }, amount, OFFERS)).toEqual({
      preview,
      apply,
      action: { CreateToken: { token: { Card: { card: offer.card, index: offer.index } }, count: amount } },
    })
  })

  it('refuses a place the list no longer has', () => {
    expect(tokenPreview({ ...START_TOKEN, token: cardChoice(5) }, 1, OFFERS)).toEqual({
      problem: 'Choose a token from the list.',
    })
  })
})

describe('the Tokens row with the seat’s card tokens', () => {
  async function openRow(offers: readonly TokenOfferView[]) {
    const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
    const table = renderTable({
      seats: 2,
      options: { viewport: { width: 1600, height: 1000 } },
      ui: <CommandsPanel />,
      session: { manual: (action, reply) => (reply?.({ ok: true }), manual(action)) },
      state: (first) => ({ ...first, view: first.view && { ...first.view, tokens: [...offers] } }),
    })
    const panel = screen.getByRole('region', { name: 'Commands' })
    const toggle = within(panel)
      .getAllByRole('button', { name: /^Tokens/ })
      .find((button) => button.hasAttribute('aria-expanded'))
    await table.user.click(toggle as HTMLElement)
    return { ...table, manual, form: within(within(panel).getByRole('group', { name: 'Tokens' })) }
  }

  it('groups them between the common tokens and "Custom creature…"', async () => {
    const { form } = await openRow(OFFERS)
    const list = form.getByRole('combobox', { name: 'Token' })
    const groups = [...list.querySelectorAll('optgroup')].map((group) => group.label)
    expect(groups).toEqual(['Common tokens', 'Your cards make'])
    const mine = [...(list.querySelectorAll('optgroup')[1]?.querySelectorAll('option') ?? [])]
    expect(mine.map((option) => option.textContent)).toEqual(['Eldrazi Scion 1/1', 'Clue'])
    expect([...list.querySelectorAll('option')].at(-1)).toHaveTextContent('Custom creature…')
  })

  it('shows the chosen token’s abilities and sends the card token on Apply', async () => {
    const { user, form, manual, container } = await openRow(OFFERS)
    expect(container.querySelector('[data-slot="card-token"]')).toBeNull()
    await user.selectOptions(form.getByRole('combobox', { name: 'Token' }), cardChoice(0))
    const shown = container.querySelector('[data-slot="card-token"]')
    expect(shown).toHaveTextContent('Made by Abstruse Interference')
    expect(shown).toHaveTextContent('Sacrifice this token: Add {C}.')
    await user.click(form.getByRole('button', { name: 'Create 1 Eldrazi Scion 1/1 token' }))
    expect(manual).toHaveBeenCalledWith({
      CreateToken: { token: { Card: { card: 'Abstruse Interference', index: 0 } }, count: 1 },
    })
  })

  it('names the card in the option when two cards make a token of the same name and size', async () => {
    const { form } = await openRow([CLUE, { ...CLUE, card: 'Tireless Tracker' }, SCION])
    const list = form.getByRole('combobox', { name: 'Token' })
    const mine = [...(list.querySelectorAll('optgroup')[1]?.querySelectorAll('option') ?? [])]
    expect(mine.map((option) => option.textContent)).toEqual([
      'Clue (Angelic Sleuth)',
      'Clue (Tireless Tracker)',
      'Eldrazi Scion 1/1',
    ])
  })

  it('has no group for a seat whose cards make none', async () => {
    const { form } = await openRow([])
    const list = form.getByRole('combobox', { name: 'Token' })
    expect([...list.querySelectorAll('optgroup')].map((group) => group.label)).toEqual(['Common tokens'])
  })
})

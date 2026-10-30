// Goal: the Tokens row of the Commands panel also makes a custom creature: "Custom creature…" asks for a name, a power and a toughness, builds the same CreateToken action the card menu's form built (a creature with no colour, the name as its one subtype when it is letters only), says what is wrong before Apply, and leaves the common tokens as they were.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ManualAction } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { CommandsPanel } from './CommandsPanel'
import { CUSTOM, START_TOKEN, tokenPreview } from './customToken'
import type { TokenDraft } from './customToken'

const custom = (more: Partial<TokenDraft> = {}): TokenDraft => ({
  ...START_TOKEN,
  token: CUSTOM,
  name: 'Knight',
  power: 2,
  toughness: 3,
  ...more,
})

const creature = (name: string, power: number, toughness: number, subtypes: string[]) => ({
  Custom: { name, colors: [], types: ['Creature'], subtypes, power, toughness },
})

describe('the preview of a custom creature', () => {
  it.each<[string, TokenDraft, number, string, string, object]>([
    [
      'one named in letters',
      custom({ name: ' Knight ' }),
      1,
      '1 Knight 2/3 creature token enter under your control',
      'Create 1 Knight 2/3 token',
      creature('Knight', 2, 3, ['Knight']),
    ],
    [
      'several of them',
      custom({ name: 'Elf Druid', power: 0, toughness: 99 }),
      3,
      '3 Elf Druid 0/99 creature tokens enter under your control',
      'Create 3 Elf Druid 0/99 tokens',
      creature('Elf Druid', 0, 99, ['Elf Druid']),
    ],
    [
      'a name with a digit, which is no subtype',
      custom({ name: 'Elf 2', power: 1, toughness: 1 }),
      1,
      '1 Elf 2 1/1 creature token enter under your control',
      'Create 1 Elf 2 1/1 token',
      creature('Elf 2', 1, 1, []),
    ],
  ])('says and builds %s', (_name, draft, amount, preview, apply, token) => {
    expect(tokenPreview(draft, amount)).toEqual({
      preview,
      apply,
      action: { CreateToken: { token, count: amount } },
    })
  })

  const STATS = 'Power and toughness are whole numbers from 0 to 99.'
  const NAME = 'Name the token in 1 to 40 characters.'
  it.each<[string, TokenDraft, string]>([
    ['no name', custom({ name: ' ' }), NAME],
    ['a name of 41 characters', custom({ name: 'x'.repeat(41) }), NAME],
    ['a power of 100', custom({ power: 100 }), STATS],
    ['a negative toughness', custom({ toughness: -1 }), STATS],
    ['a power that is not a whole number', custom({ power: 1.5 }), STATS],
    ['an emptied power', custom({ power: Number.NaN }), STATS],
  ])('refuses %s and sends nothing', (_name, draft, problem) => {
    expect(tokenPreview(draft, 1)).toEqual({ problem })
  })

  it('keeps a common token as it was', () => {
    expect(tokenPreview({ ...START_TOKEN, token: 'Soldier' }, 2)).toEqual({
      preview: '2 Soldier tokens enter under your control',
      apply: 'Create 2 Soldiers',
      action: { CreateToken: { token: { Common: 'Soldier' }, count: 2 } },
    })
  })
})

describe('the Tokens row', () => {
  async function openRow() {
    const manual = vi.fn<(action: ManualAction) => boolean>(() => true)
    const table = renderTable({
      seats: 2,
      options: { viewport: { width: 1600, height: 1000 } },
      ui: <CommandsPanel />,
      session: { manual: (action, reply) => (reply?.({ ok: true }), manual(action)) },
    })
    const panel = screen.getByRole('region', { name: 'Commands' })
    const toggle = within(panel)
      .getAllByRole('button', { name: /^Tokens/ })
      .find((button) => button.hasAttribute('aria-expanded'))
    await table.user.click(toggle as HTMLElement)
    return { ...table, manual, form: within(within(panel).getByRole('group', { name: 'Tokens' })) }
  }

  it('lists "Custom creature…" last and asks for the creature only once it is chosen', async () => {
    const { user, form } = await openRow()
    const list = form.getByRole('combobox', { name: 'Token' })
    expect([...list.querySelectorAll('option')].at(-1)).toHaveTextContent('Custom creature…')
    expect(form.queryByLabelText('Name of the token')).toBeNull()
    await user.selectOptions(list, CUSTOM)
    expect(form.getByLabelText('Name of the token')).toHaveValue('')
    expect(form.getByLabelText('Name of the token')).toHaveFocus()
    expect(form.getByLabelText('Power')).toHaveValue(1)
    expect(form.getByLabelText('Toughness')).toHaveValue(1)
  })

  it('says what is missing until the creature has a name, then sends the custom token on Apply', async () => {
    const { user, form, manual } = await openRow()
    await user.selectOptions(form.getByRole('combobox', { name: 'Token' }), CUSTOM)
    const preview = (): string =>
      form.getByText(/./, { selector: '[data-slot="command-preview"]' }).textContent ?? ''
    expect(preview()).toBe('Name the token in 1 to 40 characters.')
    await user.click(form.getByRole('button', { name: 'Apply' }))
    expect(manual).not.toHaveBeenCalled()
    await user.type(form.getByLabelText('Name of the token'), 'Knight')
    await user.clear(form.getByLabelText('Power'))
    await user.type(form.getByLabelText('Power'), '2')
    await user.clear(form.getByLabelText('Toughness'))
    await user.type(form.getByLabelText('Toughness'), '3')
    await user.click(form.getByRole('button', { name: '2' }))
    expect(preview()).toBe('2 Knight 2/3 creature tokens enter under your control')
    await user.click(form.getByRole('button', { name: 'Create 2 Knight 2/3 tokens' }))
    expect(manual).toHaveBeenCalledWith({
      CreateToken: { token: creature('Knight', 2, 3, ['Knight']), count: 2 },
    })
  })

  it('still creates a common token with the list alone', async () => {
    const { user, form, manual } = await openRow()
    await user.selectOptions(form.getByRole('combobox', { name: 'Token' }), 'Soldier')
    await user.click(form.getByRole('button', { name: 'Create 1 Soldier' }))
    expect(manual).toHaveBeenCalledWith({ CreateToken: { token: { Common: 'Soldier' }, count: 1 } })
  })
})

// Goal: the amount field of the Commands panel and the Card group is one component: a signed spec's quick amounts add to the pending change and a count's set it, both clamped to the spec; Page Up and Page Down move it by 5; the quick amounts are named by the change alone or after the field; an emptied field reads as nothing.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it } from 'vitest'
import { AmountField } from './AmountField'
import type { AmountSpec } from './commands'

const SIGNED: AmountSpec = { signed: true, min: -20, max: 20, presets: [-5, -1, 1, 5], start: 0 }
const COUNT: AmountSpec = { signed: false, min: 1, max: 10, presets: [1, 2, 3], start: 1 }

function Field({
  spec,
  start,
  plain = false,
  label = 'Change',
}: {
  spec: AmountSpec
  start: number
  plain?: boolean
  label?: string
}) {
  const [value, setValue] = useState(start)
  return <AmountField spec={spec} value={value} onChange={setValue} label={label} plainPresets={plain} />
}

const field = (name = 'Change'): HTMLInputElement =>
  screen.getByRole<HTMLInputElement>('spinbutton', { name })

describe('the quick amounts', () => {
  it.each([
    [
      'signed, named after the field',
      SIGNED,
      false,
      ['Change: take 5', 'Change: take 1', 'Change: add 1', 'Change: add 5'],
    ],
    ['signed, named by the change', SIGNED, true, ['Take 5', 'Take 1', 'Add 1', 'Add 5']],
    ['a count', COUNT, true, ['1', '2', '3']],
    ['a count, named after the field', COUNT, false, ['1', '2', '3']],
  ])('name %s', (_name, spec, plain, names) => {
    render(<Field spec={spec} start={spec.start} plain={plain} />)
    expect(screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual(names)
  })

  it.each([
    ['adds to the change', SIGNED, 3, 'Add 5', 8],
    ['takes from the change', SIGNED, 3, 'Take 5', -2],
    ['stops at the top', SIGNED, 18, 'Add 5', 20],
    ['stops at the bottom', SIGNED, -18, 'Take 5', -20],
    ['sets a count', COUNT, 7, '2', 2],
  ])('%s', async (_name, spec, start, button, value) => {
    render(<Field spec={spec} start={start} plain />)
    await userEvent.setup().click(screen.getByRole('button', { name: button }))
    expect(field()).toHaveValue(value)
  })
})

describe('the field', () => {
  it.each([
    ['PageUp', 3, 8],
    ['PageDown', 3, -2],
    ['PageUp', 18, 20],
    ['PageDown', -18, -20],
  ])('%s moves it by 5 inside the spec', async (key, start, value) => {
    render(<Field spec={SIGNED} start={start} />)
    field().focus()
    await userEvent.setup().keyboard(`{${key}}`)
    expect(field()).toHaveValue(value)
  })

  it('takes a typed number', async () => {
    render(<Field spec={COUNT} start={1} label="How many" />)
    const user = userEvent.setup()
    await user.clear(field('How many'))
    await user.type(field('How many'), '4')
    expect(field('How many')).toHaveValue(4)
  })

  it('reads an emptied field as nothing and starts a Page Up from zero', async () => {
    render(<Field spec={SIGNED} start={3} />)
    const user = userEvent.setup()
    await user.clear(field())
    expect(field()).toHaveValue(null)
    await user.keyboard('{PageUp}')
    expect(field()).toHaveValue(5)
  })

  it.each([
    [SIGNED, 'text'],
    [COUNT, 'numeric'],
  ])('asks for the keyboard that has a minus sign only when the spec takes one', (spec, mode) => {
    render(<Field spec={spec} start={spec.start} />)
    expect(field()).toHaveAttribute('inputmode', mode)
  })
})

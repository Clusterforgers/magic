// Goal: a pill shows its words in the tone asked for, marks a chosen one with more than colour, and answers hover only when it is a control, so a plain label never looks pressable.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Pill } from './Pill'
import type { PillTone } from './Pill'

const TONES: ReadonlyArray<[PillTone, string]> = [
  ['neutral', 'bg-muted'],
  ['success', 'text-success'],
  ['warning', 'text-warning'],
  ['turn-you', 'bg-turn-you'],
  ['turn-them', 'text-turn-them'],
]

describe('Pill', () => {
  it('is a neutral span with its words', () => {
    render(<Pill>you</Pill>)
    const pill = screen.getByText('you')
    expect(pill.tagName).toBe('SPAN')
    expect(pill).toHaveAttribute('data-slot', 'pill')
    expect(pill).toHaveAttribute('data-tone', 'neutral')
  })

  it.each(TONES)('draws the %s tone with its own colours', (tone, expected) => {
    render(<Pill tone={tone}>Label</Pill>)
    const pill = screen.getByText('Label')
    expect(pill).toHaveAttribute('data-tone', tone)
    expect(pill).toHaveClass(expected)
  })

  it('marks a chosen pill with a ring and a data flag, not only a colour', () => {
    render(
      <>
        <Pill selected>On</Pill>
        <Pill>Off</Pill>
      </>,
    )
    expect(screen.getByText('On')).toHaveAttribute('data-selected')
    expect(screen.getByText('On')).toHaveClass('data-selected:ring-2')
    expect(screen.getByText('Off')).not.toHaveAttribute('data-selected')
  })

  it('answers hover only once it is rendered as a control', () => {
    render(
      <>
        <Pill>Plain</Pill>
        <Pill render={<button type="button" />}>Filter</Pill>
      </>,
    )
    expect(screen.getByText('Plain')).not.toHaveAttribute('data-interactive')
    const control = screen.getByRole('button', { name: 'Filter' })
    expect(control).toHaveAttribute('data-interactive')
    expect(control).toHaveAttribute('data-slot', 'pill')
  })

  it('keeps the class names and attributes it is given', () => {
    render(
      <Pill tone="success" data-slot="seat-state" className="shrink-0 extra">
        Ready
      </Pill>,
    )
    const pill = screen.getByText('Ready')
    expect(pill).toHaveAttribute('data-slot', 'seat-state')
    expect(pill).toHaveClass('extra', 'text-success')
  })
})

/** Goal: mana that outlasts the step is counted by kind with the words of when it empties, and the pool on the table and in the payment marks it `kept` while other mana stays unmarked. */
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ManaPool as Pool, ManaType } from '@/protocol/generated'
import { keptOf } from './manaKept'
import { ManaPool } from './ManaPool'
import { PaymentSummary } from './prompt/PaymentSummary'

type Six = Pool['counts']
const NONE: Six = [0, 0, 0, 0, 0, 0]
const pool = (counts: Six, turn: Six = NONE, combat: Six = NONE): Pool => ({
  counts,
  kept: { turn_end: turn, combat_end: combat },
})

describe('keptOf', () => {
  it.each<[string, Pool, ManaType, string | null]>([
    ['nothing kept', pool([0, 0, 0, 0, 2, 0]), 'Green', null],
    ['a pool without the field', { counts: [0, 0, 0, 0, 2, 0] }, 'Green', null],
    [
      'green to the end of the turn',
      pool([0, 0, 0, 0, 2, 0], [0, 0, 0, 0, 1, 0]),
      'Green',
      'kept: 1 until end of turn',
    ],
    ['another kind kept', pool([1, 0, 0, 0, 2, 0], [0, 0, 0, 0, 1, 0]), 'White', null],
    [
      'red to the end of combat and of the turn',
      pool([0, 0, 0, 3, 0, 0], [0, 0, 0, 1, 0, 0], [0, 0, 0, 2, 0, 0]),
      'Red',
      'kept: 1 until end of turn, 2 until end of combat',
    ],
    [
      'colourless to the end of combat',
      pool([0, 0, 0, 0, 0, 1], NONE, [0, 0, 0, 0, 0, 1]),
      'Colorless',
      'kept: 1 until end of combat',
    ],
  ])('%s', (_name, given, type, words) => {
    expect(keptOf(given, type)?.words ?? null).toBe(words)
  })
})

describe('the kept mark', () => {
  const given = pool([0, 0, 0, 2, 1, 0], [0, 0, 0, 0, 1, 0])

  it('marks the kept kind in the table pool and names it for a screen reader', () => {
    const { container } = render(<ManaPool pool={given} />)
    expect([...container.querySelectorAll('[data-kept]')].map((el) => el.getAttribute('data-mana'))).toEqual([
      'Green',
    ])
    expect(screen.getByText('1 green, kept: 1 until end of turn')).toBeInTheDocument()
  })

  it('marks the kept kind on the payment pool button', () => {
    render(
      <PaymentSummary
        cost={[{ Generic: 1 }]}
        owed={[{ Generic: 1 }]}
        covered={false}
        pool={given}
        spends={[]}
        nameOf={() => 'a permanent'}
        canSpend={() => true}
        onSpendPool={() => undefined}
        onRemove={() => undefined}
        onClear={() => undefined}
      />,
    )
    expect(
      screen.getByRole('button', { name: 'Spend green from your pool, 1 left, kept: 1 until end of turn' }),
    ).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Spend red from your pool, 2 left' })).toBeInTheDocument()
    expect(document.querySelectorAll('[data-mark="kept"]')).toHaveLength(1)
  })
})

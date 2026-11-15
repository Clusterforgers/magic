// Goal: at a grid of opponents every board's lands, yours too, fold into its pod's lands chip (count, tapped, names), the board then draws no land, a press on the chip lays them out until pressed again, priority's mana marks leave them folded while a prompt beyond priority that marks a land lays them out, the split counts them folded only then, and a duel and a table that does not fold keep their lands laid out.
import { fireEvent } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { foldsLands } from './boardSplit'
import { promptKey } from './boardZoom'
import { initialPlayState } from '@/play/store'
import { decisionFor } from '@/test-utils/decisions'
import { landNames, landTally, landWords } from './landFold'
import { arrangeSeats } from './layout'

describe('the tally', () => {
  const objects = {
    1: { name: 'Island', permanent: { tapped: true } },
    2: { name: 'Island', permanent: { tapped: false } },
    3: { name: 'Command Tower', permanent: { tapped: true } },
  } as unknown as Parameters<typeof landTally>[1]

  it('counts the lands, the tapped ones and the names, most first', () => {
    const tally = landTally([1, 2, 3, 9], objects)
    expect(tally).toEqual({
      count: 3,
      tapped: 2,
      names: [
        { name: 'Island', count: 2 },
        { name: 'Command Tower', count: 1 },
      ],
    })
    expect(landNames(tally.names)).toBe('Island ×2, Command Tower')
  })

  it.each([
    [1, 0, '1 land'],
    [14, 0, '14 lands'],
    [14, 5, '14 lands, 5 tapped'],
  ])('says %i lands with %i tapped as "%s"', (count, tapped, words) => {
    expect(landWords({ count, tapped })).toBe(words)
  })
})

describe('the fold at the table', () => {
  const board = (container: HTMLElement, seat: number): HTMLElement =>
    must(container.querySelector<HTMLElement>(`[data-battlefield-seat="${seat}"]`))
  const lands = (container: HTMLElement, seat: number): Element | null =>
    board(container, seat).querySelector('[data-lane="lands"]')

  it('folds every opponent of a grid into its chip and lays one out on a press', () => {
    const { container } = renderTable({ seats: 4, options: { foldLands: true } })
    for (const seat of [1, 2, 3]) {
      expect(lands(container, seat)).toBeNull()
      const chip = must(
        container.querySelector<HTMLElement>(`[data-seat="${seat}"] [data-slot="land-summary"]`),
      )
      expect(chip).toHaveAttribute('aria-expanded', 'false')
      expect(chip.getAttribute('aria-label')).toMatch(/^Show .+'s lands: \d+ lands/)
    }
    // Your own lands fold into your pod's chip too.
    expect(lands(container, 0)).toBeNull()
    expect(container.querySelector('[data-seat="0"] [data-slot="land-summary"]')).not.toBeNull()
    const chip = must(container.querySelector<HTMLElement>('[data-seat="1"] [data-slot="land-summary"]'))
    fireEvent.click(chip)
    expect(chip).toHaveAttribute('aria-expanded', 'true')
    expect(lands(container, 1)).not.toBeNull()
    expect(lands(container, 2)).toBeNull()
    fireEvent.click(chip)
    expect(lands(container, 1)).toBeNull()
  })

  it.each([
    ['a duel', 2, true],
    ['a table that does not fold', 4, false],
  ] as const)('keeps the lands of %s laid out, with no chip', (_, seats, fold) => {
    const { container } = renderTable({ seats, options: { foldLands: fold } })
    expect(lands(container, 1)).not.toBeNull()
    expect(container.querySelector('[data-slot="land-summary"]')).toBeNull()
  })

  it.each([
    [4, 'grid', true, true],
    [4, 'grid', false, false],
    [4, 'focus', true, false],
    [2, 'grid', true, false],
  ] as const)(
    'with %i seats in %s mode, folding %s, counts the lands folded in the split: %s',
    (seats, mode, fold, folds) => {
      expect(foldsLands(arrangeSeats({ seatCount: seats, viewer: 0, mode }), fold)).toBe(folds)
    },
  )

  it.each([
    [2, 'grid', true, true],
    [4, 'focus', true, true],
    [4, 'focus', false, false],
  ] as const)(
    'on a phone with %i seats in %s mode, folding %s, folds every board: %s',
    (seats, mode, fold, folds) => {
      expect(foldsLands(arrangeSeats({ seatCount: seats, viewer: 0, mode }), fold, true)).toBe(folds)
    },
  )
})

describe('what lays folded lands out', () => {
  it.each([
    ['no decision', null, null],
    ['priority', 'Priority', null],
    ['a choice of targets', 'ChooseTargets', '7'],
    ['a sacrifice', 'ChooseCards', '7'],
  ] as const)('reads %s as %s', (_, kind, key) => {
    const pending = kind === null ? null : { id: 7, decision: decisionFor(kind) }
    expect(promptKey({ ...initialPlayState(), pending })).toBe(key)
  })
})

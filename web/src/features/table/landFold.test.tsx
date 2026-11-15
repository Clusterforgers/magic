// Goal: a board that carries a lands chip (every board of a grid, every board of a phone) shows its lands by default on a desktop and folds them on a phone, the table menu's "Show lands" switch turns every board and keeps the choice in this browser, the chip (count and tapped folded, "Hide lands" open, names on hover) turns one board, priority's mana marks leave folded lands alone while a prompt beyond priority that marks a land lays them out, the split counts the lands only where they are laid out, and a duel and a table that does not fold keep their lands laid out.
import { fireEvent, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { foldsLands } from './boardSplit'
import { promptKey } from './boardZoom'
import { initialPlayState } from '@/play/store'
import { decisionFor } from '@/test-utils/decisions'
import { landNames, landTally, landWords } from './landFold'
import { LANDS_KEY, landsShownBy, setLandsShown } from './landPrefs'
import { arrangeSeats } from './layout'

beforeEach(() => {
  window.localStorage.clear()
})

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

  const chipOf = (container: HTMLElement, seat: number): HTMLElement =>
    must(container.querySelector<HTMLElement>(`[data-seat="${seat}"] [data-slot="land-summary"]`))

  it('folds every board of a grid into its chip once the lands are folded, and lays one out on a press', () => {
    setLandsShown(false)
    const { container } = renderTable({ seats: 4, options: { foldLands: true } })
    for (const seat of [1, 2, 3]) {
      expect(lands(container, seat)).toBeNull()
      const chip = chipOf(container, seat)
      expect(chip).toHaveAttribute('aria-expanded', 'false')
      expect(chip.getAttribute('aria-label')).toMatch(/^Show .+'s lands: \d+ lands/)
    }
    // Your own lands fold into your pod's chip too.
    expect(lands(container, 0)).toBeNull()
    expect(chipOf(container, 0)).toBeInTheDocument()
    const chip = chipOf(container, 1)
    fireEvent.click(chip)
    expect(chip).toHaveAttribute('aria-expanded', 'true')
    expect(lands(container, 1)).not.toBeNull()
    expect(lands(container, 2)).toBeNull()
    fireEvent.click(chip)
    expect(lands(container, 1)).toBeNull()
  })

  it('shows the lands of every board of a desktop grid by default, each chip ready to hide its own', () => {
    const { container } = renderTable({ seats: 4, options: { foldLands: true } })
    for (const seat of [0, 1, 2, 3]) expect(lands(container, seat)).not.toBeNull()
    const chip = chipOf(container, 1)
    expect(chip).toHaveAttribute('aria-expanded', 'true')
    expect(chip).toHaveTextContent('Hide lands')
    expect(chip.getAttribute('aria-label')).toMatch(/^Hide .+'s lands: \d+ lands/)
    fireEvent.click(chip)
    expect(lands(container, 1)).toBeNull()
    expect(lands(container, 2)).not.toBeNull()
    expect(chip).toHaveTextContent(/^Lands \d+/)
    expect(chip.getAttribute('title')).toMatch(/^Show lands: \S/)
  })

  it('folds the boards of a phone by default, the focused one and yours', () => {
    const { container } = renderTable({
      seats: 4,
      options: { foldLands: true, viewport: { width: 390, height: 844 } },
    })
    expect(container.querySelector('[data-lane="lands"]')).toBeNull()
    expect(container.querySelector('[data-slot="board-lands"] [data-slot="land-summary"]')).not.toBeNull()
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

describe('the Show lands choice', () => {
  const board = (container: HTMLElement, seat: number): HTMLElement =>
    must(container.querySelector<HTMLElement>(`[data-battlefield-seat="${seat}"]`))
  const laid = (container: HTMLElement): number[] =>
    [0, 1, 2, 3].filter((seat) => board(container, seat).querySelector('[data-lane="lands"]') !== null)
  const rows = (container: HTMLElement): string =>
    must(container.querySelector<HTMLElement>('[data-slot="table-main"]')).style.gridTemplateRows

  it.each([
    [null, false, true],
    [null, true, false],
    ['show', true, true],
    ['fold', false, false],
  ] as const)('reads the choice %s on a phone %s as shown: %s', (choice, phone, shown) => {
    expect(landsShownBy(choice, phone)).toBe(shown)
  })

  it('turns every board from the table menu, reads the switch as the table shows and keeps the choice', async () => {
    const { container, user } = renderTable({ seats: 4, options: { foldLands: true } })
    expect(laid(container)).toEqual([0, 1, 2, 3])
    await user.click(screen.getByRole('button', { name: 'Table menu' }))
    const toggle = await screen.findByRole('switch', { name: 'Show lands' })
    expect(toggle).toBeChecked()
    await user.click(toggle)
    expect(toggle).not.toBeChecked()
    expect(laid(container)).toEqual([])
    expect(window.localStorage.getItem(LANDS_KEY)).toBe('fold')
    await user.click(toggle)
    expect(laid(container)).toEqual([0, 1, 2, 3])
    expect(window.localStorage.getItem(LANDS_KEY)).toBe('show')
  })

  it('lets a board that was turned follow the next change of the table', async () => {
    const { container, user } = renderTable({ seats: 4, options: { foldLands: true } })
    fireEvent.click(must(container.querySelector<HTMLElement>('[data-seat="1"] [data-slot="land-summary"]')))
    expect(laid(container)).toEqual([0, 2, 3])
    await user.click(screen.getByRole('button', { name: 'Table menu' }))
    await user.click(await screen.findByRole('switch', { name: 'Show lands' }))
    expect(laid(container)).toEqual([])
    await user.click(screen.getByRole('switch', { name: 'Show lands' }))
    expect(laid(container)).toEqual([0, 1, 2, 3])
  })

  it.each([
    ['show', [0, 1, 2, 3]],
    ['fold', []],
  ] as const)('opens with the kept choice %s: lands laid out on boards %j', (choice, boards) => {
    window.localStorage.setItem(LANDS_KEY, choice)
    const { container } = renderTable({ seats: 4, options: { foldLands: true } })
    expect(laid(container)).toEqual(boards)
  })

  it('shows lands on a phone once the choice says so', () => {
    window.localStorage.setItem(LANDS_KEY, 'show')
    const { container } = renderTable({
      seats: 4,
      options: { foldLands: true, viewport: { width: 390, height: 844 } },
    })
    expect(container.querySelectorAll('[data-lane="lands"]').length).toBeGreaterThan(0)
  })

  it('splits the rows with the lands counted only where they are laid out', () => {
    window.localStorage.setItem(LANDS_KEY, 'fold')
    const folded = renderTable({ seats: 4, options: { foldLands: true } })
    const foldedRows = rows(folded.container)
    folded.unmount()
    window.localStorage.setItem(LANDS_KEY, 'show')
    const shown = renderTable({ seats: 4, options: { foldLands: true } })
    expect(rows(shown.container)).not.toBe(foldedRows)
  })

  it('keeps the rows of a table that does not fold as a shown choice has them', () => {
    window.localStorage.setItem(LANDS_KEY, 'show')
    const chips = renderTable({ seats: 4, options: { foldLands: true } })
    const withChips = rows(chips.container)
    chips.unmount()
    const plain = renderTable({ seats: 4, options: { foldLands: false } })
    expect(rows(plain.container)).toBe(withChips)
  })
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

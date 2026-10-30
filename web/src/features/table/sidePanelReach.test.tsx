// Goal: an open rail panel or side sheet is drawn into an overlay of the main area that spans the rows from the opponents down (to the shared strip for a sheet with room, else to your board) and never the top bar or the hand and controls row, so the pass control and the hand stay pressable with it open; while a prompt waits in the shared strip (any decision of this seat but Priority) the overlay stops above the prompt sheet and returns to its rows once the decision is answered; the sheet leaves the page live, and "Pick a card" takes the panel away and brings it back with the card chosen.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf } from '@/test-utils/combatTable'
import { renderTable } from '@/test-utils/tableHarness'
import { regionSizes, SIDE_DECISION_MIN_PX, sidePanelRows } from './layout'
import type { Viewport } from './layout'
import { promptInStrip } from './prompt/ownDecision'

const SIZES: ReadonlyArray<[string, Viewport, string]> = [
  ['1920x1080 column', { width: 1920, height: 1080 }, '2 / 5'],
  ['1024x768 rail', { width: 1024, height: 768 }, '2 / 5'],
  ['1100x800 rail', { width: 1100, height: 800 }, '2 / 5'],
  ['768x1024 tablet sheet', { width: 768, height: 1024 }, '2 / 4'],
  ['640x768 sheet', { width: 640, height: 768 }, '2 / 4'],
  ['640x700 short sheet', { width: 640, height: 700 }, '2 / 5'],
  ['390x844 phone sheet', { width: 390, height: 844 }, '2 / 4'],
  ['390x640 short phone sheet', { width: 390, height: 640 }, '2 / 5'],
  ['844x390 phone on its side', { width: 844, height: 390 }, '2 / 5'],
]

describe('the rows an open panel may take', () => {
  it.each(SIZES)('%s', (_name, viewport, rows) => {
    expect(sidePanelRows(regionSizes(viewport))).toBe(rows)
  })

  it('never starts above the opponents or ends below your board', () => {
    for (const [, viewport] of SIZES) {
      const [from, to] = sidePanelRows(regionSizes(viewport)).split(' / ').map(Number)
      expect(from).toBe(2)
      expect(to).toBeLessThanOrEqual(5)
    }
  })
})

const NAME = 'Commands, log, chat and stack'
const opener = () => screen.getByRole('button', { name: /^Commands, log, chat and stack/ })
const overlay = () => document.querySelector<HTMLElement>('[data-slot="side-overlay"]') as HTMLElement
const pass = () => document.querySelector('[data-region="controls"] [data-slot="primary-action"]')

describe.each([
  ['a rail', { width: 1100, height: 800 }, '2 / 5'],
  ['a phone sheet', { width: 390, height: 844 }, '2 / 4'],
])('%s open', (_name, viewport, rows) => {
  const open = async () => {
    const table = renderTable({ seats: 2, options: { viewport } })
    if (viewport.width < 1024) await table.user.click(opener())
    else await table.user.click(screen.getByRole('tab', { name: 'Commands' }))
    return table
  }

  it('draws the panel in the overlay of the opponents down, outside the bar and the dock rows', async () => {
    await open()
    expect(overlay().style.gridRow).toBe(rows)
    expect(overlay().closest('[data-slot="table-main"]')).not.toBeNull()
    expect(within(overlay()).getByRole('region', { name: 'Commands' })).toBeInTheDocument()
    for (const area of ['bar', 'hand', 'controls']) {
      expect(document.querySelector(`[data-area="${area}"]`)?.contains(overlay())).toBe(false)
      expect(overlay().contains(document.querySelector(`[data-area="${area}"]`))).toBe(false)
    }
  })

  it('leaves the page live: the controls dock and the top bar are not inert', async () => {
    await open()
    expect(pass()).not.toBeNull()
    expect(pass()?.closest('[inert], [aria-hidden="true"]')).toBeNull()
    expect(screen.getByRole('button', { name: /^Table menu|^Menu/ })).toBeVisible()
  })
})

describe('the sheet', () => {
  it('stays open when the table around it is pressed, and its button toggles it', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: { width: 390, height: 844 } } })
    await user.click(opener())
    expect(screen.getByRole('dialog', { name: NAME })).toBeInTheDocument()
    await user.click(document.querySelector('[data-area="own"]') as HTMLElement)
    expect(screen.getByRole('dialog', { name: NAME })).toBeInTheDocument()
    await user.click(opener())
    expect(opener()).toHaveAttribute('aria-expanded', 'false')
  })
})

describe.each([
  ['a rail', { width: 1100, height: 800 }],
  ['a phone sheet', { width: 390, height: 844 }],
])('%s and a card to pick', (_name, viewport) => {
  it('goes away while the card is waited for and returns with the card chosen', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport } })
    if (viewport.width < 1024) await user.click(opener())
    else await user.click(screen.getByRole('tab', { name: 'Commands' }))
    await user.click(document.querySelector('[data-command="pick"]') as HTMLElement)
    expect(screen.queryByRole('region', { name: 'Commands' })).toBeNull()
    expect(document.querySelector('[data-slot="pick-banner"]')).not.toBeNull()
    await user.click(document.querySelector('[data-area="own"] button[data-object-id]') as HTMLElement)
    expect(await screen.findByRole('region', { name: 'Commands' })).toBeInTheDocument()
    expect(document.querySelector('[data-slot="card-picked"]')).not.toBeNull()
    expect(document.querySelector('[data-slot="pick-banner"]')).toBeNull()
  })
})

describe('the rows while a prompt waits in the strip', () => {
  it.each(SIZES)('%s stops above the strip', (_name, viewport) => {
    expect(sidePanelRows(regionSizes(viewport), true)).toBe('2 / 3')
  })
})

const SOURCE = must(creaturesOf(fixtureOf(2), 0)[0])
const yesNo = (player: number) => ({ id: 7, decision: { player, kind: { YesNo: { source: SOURCE } } } })

describe('promptInStrip', () => {
  it.each<[string, 'priority' | ReturnType<typeof yesNo> | null, number | null, boolean]>([
    ['a Priority decision of this seat, whose buttons are in the dock', 'priority', 0, false],
    ['a yes-or-no question of this seat', yesNo(0), 0, true],
    ['a question of another seat', yesNo(1), 0, false],
    ['no decision', null, 0, false],
    ['a viewer with no seat', yesNo(0), null, false],
  ])('is false or true for %s', (_what, pending, seat, expected) => {
    const { store } = renderTable({ seats: 2 })
    const own = store.getState()
    const given = pending === 'priority' ? own.pending : pending
    expect(promptInStrip({ ...own, seat, pending: given })).toBe(expected)
  })
})

describe.each([
  ['a rail', { width: 1100, height: 800 }],
  ['a phone sheet', { width: 390, height: 844 }],
  ['a short phone sheet', { width: 390, height: 640 }],
])('%s with a decision to answer', (_name, viewport) => {
  const open = async () => {
    const table = renderTable({
      seats: 2,
      options: { viewport },
      state: (state) => ({ ...state, pending: yesNo(0) }),
    })
    if (viewport.width < 1024) await table.user.click(opener())
    else await table.user.click(screen.getByRole('tab', { name: 'Commands' }))
    return table
  }

  it('stops above the prompt sheet, with room kept for its tabs, and leaves the prompt outside it', async () => {
    await open()
    expect(overlay().style.gridRow).toBe('2 / 3')
    expect(overlay().dataset.lowered).toBe('true')
    expect(overlay().style.marginBottom).toBe('var(--sheet-overhang, 0px)')
    expect(overlay().style.minHeight).toBe(`${SIDE_DECISION_MIN_PX}px`)
    expect(within(overlay()).getByRole('region', { name: 'Commands' })).toBeInTheDocument()
    const prompt = screen.getByRole('button', { name: 'Yes' })
    expect(overlay().contains(prompt)).toBe(false)
    expect(document.querySelector('[data-region="shared"]')?.contains(prompt)).toBe(true)
  })

  it('returns to its full rows once the decision is answered', async () => {
    const table = await open()
    table.update(() => ({ pending: null }))
    expect(overlay().dataset.lowered).toBeUndefined()
    expect(overlay().style.gridRow).toBe(sidePanelRows(regionSizes(viewport)))
    expect(overlay().style.minHeight).toBe('')
    expect(overlay().style.marginBottom).toBe('')
  })
})

describe('a Priority decision', () => {
  it('leaves the sheet at its full rows, since its pass buttons are in the controls dock', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: { width: 390, height: 844 } } })
    await user.click(opener())
    expect(overlay().dataset.lowered).toBeUndefined()
    expect(overlay().style.gridRow).toBe('2 / 4')
  })
})

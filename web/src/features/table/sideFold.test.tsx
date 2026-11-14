// Goal: the viewer folds the side column to its icon rail and opens it again from the rail, the boards get the column's width, the choice is kept per browser (and for the page when storage is blocked), and a window too narrow for a column is not changed by it.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { BREAKPOINT_FROM, RAIL_PX, regionSizes } from './layout'
import { SIDE_FOLD_KEY, setSideFolded, sideFolded } from './sideFold'

afterEach(() => {
  act(() => setSideFolded(false))
  vi.restoreAllMocks()
})

describe('regionSizes with the column folded', () => {
  it.each([1280, 1400, 1600, 1920])('draws the rail at %i px and gives the main area its width', (width) => {
    const open = regionSizes({ width, height: 900 })
    const folded = regionSizes({ width, height: 900 }, true)
    expect(open).toMatchObject({ side: 'column', foldable: true })
    expect(folded).toMatchObject({ side: 'rail', foldable: true, sideWidth: RAIL_PX })
    expect(folded.main.width).toBe(width - RAIL_PX)
  })

  it.each([390, 800, BREAKPOINT_FROM.m])('leaves a %i px window as it was', (width) => {
    expect(regionSizes({ width, height: 900 }, true)).toEqual(regionSizes({ width, height: 900 }))
    expect(regionSizes({ width, height: 900 }).foldable).toBe(false)
  })
})

describe('the preference', () => {
  it('is kept in localStorage and read back', () => {
    expect(sideFolded()).toBe(false)
    setSideFolded(true)
    expect(localStorage.getItem(SIDE_FOLD_KEY)).toBe('rail')
    expect(sideFolded()).toBe(true)
    setSideFolded(false)
    expect(localStorage.getItem(SIDE_FOLD_KEY)).toBeNull()
  })

  it('lasts for the page when storage is blocked', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    setSideFolded(true)
    expect(sideFolded()).toBe(true)
  })
})

describe('the buttons', () => {
  it('folds the column into the rail and opens it again', () => {
    const { container } = renderTable({ seats: 4 })
    expect(container.querySelector('[data-side="column"]')).not.toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Fold the side panel' }))
    expect(container.querySelector('[data-side="column"]')).toBeNull()
    expect(container.querySelector('[data-side="rail"]')).not.toBeNull()
    expect(sideFolded()).toBe(true)
    fireEvent.click(screen.getByRole('button', { name: 'Open the side panel' }))
    expect(container.querySelector('[data-side="column"]')).not.toBeNull()
    expect(sideFolded()).toBe(false)
  })
})

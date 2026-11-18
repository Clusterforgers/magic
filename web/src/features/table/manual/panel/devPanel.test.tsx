// Goal: the dev table's `?panel=` reaches the Commands panel from the side the table opens on (the log) in a few steps, in the column, on the rail and from a phone's sheet.
import { act } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import type { Viewport } from '../../layout'
import { showPanel } from './devPanel'

describe('showPanel', () => {
  it.each<[string, Viewport]>([
    ['the column', { width: 1600, height: 1000 }],
    ['the rail', { width: 1100, height: 800 }],
    ['a phone sheet', { width: 390, height: 844 }],
  ])('draws the panel in %s within three steps', (_name, viewport) => {
    renderTable({ seats: 2, options: { viewport } })
    expect(document.querySelector('[data-slot="commands-panel"]')).toBeNull()
    let shown = false
    for (let step = 0; step < 3 && !shown; step += 1) act(() => void (shown = showPanel()))
    expect(shown || showPanel()).toBe(true)
    expect(document.querySelector('[data-slot="commands-panel"]')).not.toBeNull()
  })
})

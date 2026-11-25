// Goal: a hand or a board laid out anew for its room (its first measure on a load or a reload, every resize after) holds its cards' transitions off until two frames later, so nothing glides into the places a snapshot or a new size gives it, while a render at the same size leaves the cards free to move.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { blockAfter } from '@/test-utils/cssBlocks'
import { renderTable } from '@/test-utils/tableHarness'
import type { FitBox } from './fit'
import { STILL_ATTR, useStillOnResize } from './useZoneFit'

/** Waits for `count` animation frames. */
const frames = async (count: number): Promise<void> => {
  for (let i = 0; i < count; i += 1) await new Promise((done) => requestAnimationFrame(() => done(null)))
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('useStillOnResize', () => {
  const mount = (box: FitBox | null) => {
    const element = document.createElement('div')
    const ref = { current: element }
    const hook = renderHook(({ room }) => useStillOnResize(ref, room), { initialProps: { room: box } })
    return { element, hook }
  }

  it.each<[string, FitBox | null, FitBox]>([
    ['the first measure', null, { width: 800, height: 180 }],
    ['a wider room', { width: 800, height: 180 }, { width: 1200, height: 180 }],
    ['a taller room', { width: 800, height: 180 }, { width: 800, height: 260 }],
  ])('marks the zone still for %s and frees it two frames later', async (_, from, to) => {
    const { element, hook } = mount(from)
    await frames(3)
    expect(element.hasAttribute(STILL_ATTR)).toBe(false)
    hook.rerender({ room: to })
    expect(element.hasAttribute(STILL_ATTR)).toBe(true)
    await frames(3)
    expect(element.hasAttribute(STILL_ATTR)).toBe(false)
  })

  it('leaves the zone free on a render at the same size', async () => {
    const { element, hook } = mount({ width: 800, height: 180 })
    await frames(3)
    hook.rerender({ room: { width: 800, height: 180 } })
    expect(element.hasAttribute(STILL_ATTR)).toBe(false)
  })

  it.each([2, 4])('marks the hand dock and every board of a table of %i that has just loaded', (seats) => {
    renderTable({ seats })
    const dock = document.querySelector('[data-region="hand"]')
    expect(dock?.querySelector(`[${STILL_ATTR}]`)).not.toBeNull()
    const boards = [...document.querySelectorAll('[data-slot="battlefield-zone"]')]
    expect(boards).toHaveLength(seats)
    for (const board of boards) expect(board.hasAttribute(STILL_ATTR)).toBe(true)
  })
})

describe('the still hand in feel.css', () => {
  const css = readFileSync(join(import.meta.dirname, '../../feel.css'), 'utf8').replace(
    /\/\*[\s\S]*?\*\//g,
    '',
  )

  it('turns every transition under the mark off', () => {
    expect(blockAfter(css, `[${STILL_ATTR}] *`)).toContain('transition: none')
  })
})

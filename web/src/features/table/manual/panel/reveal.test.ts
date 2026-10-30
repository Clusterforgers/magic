//! Goal: a row that opens below or above the panel's view is scrolled into it by the least amount, with its head kept in view, and a row in view or outside a panel leaves the scroll alone.
import { describe, expect, it } from 'vitest'
import { revealRow } from './reveal'

function box(top: number, bottom: number): DOMRect {
  return {
    top,
    bottom,
    left: 0,
    right: 100,
    width: 100,
    height: bottom - top,
    x: 0,
    y: top,
    toJSON: () => ({}),
  }
}

function panelWith(
  row: [number, number],
  port: [number, number] = [0, 500],
): { panel: HTMLElement; row: HTMLElement } {
  const panel = document.createElement('section')
  panel.dataset.slot = 'commands-panel'
  const element = document.createElement('div')
  panel.append(element)
  panel.getBoundingClientRect = () => box(...port)
  element.getBoundingClientRect = () => box(...row)
  panel.scrollTop = 100
  return { panel, row: element }
}

describe('revealRow', () => {
  it.each([
    ['in view', [100, 300], 100],
    ['foot below the view', [400, 600], 200],
    ['taller than the view, head kept on top', [450, 1200], 550],
    ['head above the view', [-50, 200], 50],
  ] as const)('a row %s scrolls the panel to %#', (_, rect, scrollTop) => {
    const { panel, row } = panelWith([rect[0], rect[1]])
    revealRow(row)
    expect(panel.scrollTop).toBe(scrollTop)
  })

  it('does nothing for a row outside a panel', () => {
    const row = document.createElement('div')
    expect(() => revealRow(row)).not.toThrow()
  })
})

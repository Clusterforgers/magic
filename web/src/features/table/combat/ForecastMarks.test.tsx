// Goal: the marks a forecast puts on a card follow its outlook in every combination (a death with damage, damage that is survived, a death with none, a compact card, a card turned a quarter): a cracked grey look and a skull only for a creature that dies, a "-N" badge only for damage, placed over the card as seen, and all of it decoration that takes no pointer and is hidden from assistive technology (the words are in the card's name).
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { CreatureOutlook } from './forecast'
import { ForecastMarks } from './ForecastMarks'

const mark = (outlook: CreatureOutlook, { tapped = false, compact = false } = {}) => {
  const view = render(<ForecastMarks outlook={outlook} tapped={tapped} compact={compact} />)
  const root = view.container.querySelector<HTMLElement>('[data-slot="forecast"]')
  if (root === null) throw new Error('no marks')
  return {
    root,
    cracks: root.querySelector('[data-slot="forecast-cracks"]'),
    doomed: root.querySelector<HTMLElement>('[data-slot="forecast-doomed"]'),
    damage: root.querySelector<HTMLElement>('[data-slot="forecast-damage"]'),
  }
}

describe('ForecastMarks', () => {
  it.each<[string, CreatureOutlook, { cracks: boolean; skull: boolean; damage: string | null }]>([
    ['damage that kills', { damage: 4, dies: true }, { cracks: true, skull: true, damage: '-4' }],
    ['damage that is survived', { damage: 2, dies: false }, { cracks: false, skull: false, damage: '-2' }],
    ['one point of damage', { damage: 1, dies: false }, { cracks: false, skull: false, damage: '-1' }],
    [
      'a death with no damage (no toughness)',
      { damage: 0, dies: true },
      { cracks: true, skull: true, damage: null },
    ],
    ['a large amount of damage', { damage: 12, dies: true }, { cracks: true, skull: true, damage: '-12' }],
  ])('for %s draws the cracks, the skull and the badge that belong to it', (_name, outlook, expected) => {
    const { cracks, doomed, damage } = mark(outlook)
    expect(cracks !== null).toBe(expected.cracks)
    expect(doomed !== null).toBe(expected.skull)
    expect(damage?.textContent ?? null).toBe(expected.damage)
  })

  it('names the death in words beside the skull on a card with room', () => {
    expect(mark({ damage: 3, dies: true }).doomed).toHaveTextContent('Dies')
  })

  it('keeps the skull alone on a compact card, which has no room for words', () => {
    const { doomed, damage } = mark({ damage: 3, dies: true }, { compact: true })
    expect(doomed).not.toHaveTextContent('Dies')
    expect(doomed?.querySelector('svg')).not.toBeNull()
    expect(damage).toHaveTextContent('-3')
  })

  it('covers the whole card when it stands, and only the band the card occupies when it is turned a quarter', () => {
    const upright = mark({ damage: 1, dies: true })
    expect(upright.root).toHaveClass('inset-y-0')
    expect(upright.root.className).not.toContain('h-(--card-w)')
    const turned = mark({ damage: 1, dies: true }, { tapped: true })
    expect(turned.root.className).toContain('h-(--card-w)')
    expect(turned.root.className).toContain('top-[calc((var(--card-h)-var(--card-w))/2)]')
    expect(turned.root).not.toHaveClass('inset-y-0')
  })

  it('is decoration: hidden from assistive technology and never takes the pointer', () => {
    const { root } = mark({ damage: 5, dies: true })
    expect(root).toHaveAttribute('aria-hidden', 'true')
    expect(root).toHaveClass('pointer-events-none')
    expect(root.querySelectorAll('a, button, input, [tabindex]')).toHaveLength(0)
  })

  it('stays inside the card: its box is clipped to the card and rounded like it', () => {
    const { root } = mark({ damage: 5, dies: true })
    expect(root).toHaveClass('overflow-hidden')
    expect(root).toHaveClass('rounded-md')
  })

  it('draws the cracks as strokes that keep their width however the card is sized', () => {
    const { cracks } = mark({ damage: 5, dies: true })
    const strokes = [...(cracks?.querySelectorAll('path') ?? [])]
    expect(strokes.length).toBeGreaterThanOrEqual(2)
    for (const stroke of strokes) expect(stroke).toHaveAttribute('vector-effect', 'non-scaling-stroke')
  })
})

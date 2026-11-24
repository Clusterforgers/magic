// Goal: the marks a forecast puts on a card follow its outlook in every combination (a death with damage, damage that is survived, a death with none, damage dealt, a compact card, a card turned a quarter, a corner another mark holds): a cracked grey look and a skull only for a creature that dies, a "-N" badge only for damage taken, a sword badge only for damage dealt, each badge with its meaning in a sign or icon and an aria-label, placed over the card as seen and clear of the corner and the stat box, and none of it takes the pointer.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { CreatureOutlook } from './forecast'
import { ForecastMarks } from './ForecastMarks'

const mark = (outlook: CreatureOutlook, { tapped = false, compact = false, cornerTaken = false } = {}) => {
  const view = render(
    <ForecastMarks outlook={outlook} tapped={tapped} compact={compact} cornerTaken={cornerTaken} />,
  )
  const root = view.container.querySelector<HTMLElement>('[data-slot="forecast"]')
  if (root === null) throw new Error('no marks')
  return {
    root,
    cracks: root.querySelector('[data-slot="forecast-cracks"]'),
    doomed: root.querySelector<HTMLElement>('[data-slot="forecast-doomed"]'),
    damage: root.querySelector<HTMLElement>('[data-slot="forecast-damage"]'),
    dealt: root.querySelector<HTMLElement>('[data-slot="forecast-dealt"]'),
    row: root.querySelector<HTMLElement>('[data-slot="forecast-badges"]'),
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

  it('puts a ≈ before the damage when it depends on a choice', () => {
    expect(mark({ damage: 3, dies: false, choice: true }).damage).toHaveTextContent('≈-3')
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

  it('covers the whole card when it stands, and only the band the card occupies when it lies on its side', () => {
    const upright = mark({ damage: 1, dies: true })
    expect(upright.root).toHaveClass('inset-y-0')
    expect(upright.root.className).not.toContain('h-[calc(var(--card-w)*5/7)]')
    const turned = mark({ damage: 1, dies: true }, { tapped: true })
    expect(turned.root.className).toContain('h-[calc(var(--card-w)*5/7)]')
    expect(turned.root.className).toContain('top-[calc((var(--card-h)-var(--card-w)*5/7)/2)]')
    expect(turned.root).not.toHaveClass('inset-y-0')
  })

  it.each<[string, CreatureOutlook, Record<string, string | null>]>([
    [
      'an attacker that deals and lives',
      { damage: 0, dies: false, dealt: 4 },
      { dealt: 'Deals 4 damage', damage: null, doomed: null },
    ],
    [
      'a trade',
      { damage: 2, dies: true, dealt: 2 },
      { dealt: 'Deals 2 damage', damage: 'Takes 2 damage', doomed: 'Dies' },
    ],
    [
      'a blocker that survives',
      { damage: 1, dies: false, dealt: 3 },
      { dealt: 'Deals 3 damage', damage: 'Takes 1 damage', doomed: null },
    ],
    [
      'a creature that deals nothing',
      { damage: 3, dies: false },
      { dealt: null, damage: 'Takes 3 damage', doomed: null },
    ],
  ])(
    'labels each badge of %s for a screen reader and keeps its meaning out of colour',
    (_name, outlook, labels) => {
      const marks = mark(outlook)
      for (const slot of ['dealt', 'damage', 'doomed'] as const) {
        const badge = marks[slot]
        expect(badge?.getAttribute('aria-label') ?? null).toBe(labels[slot])
        if (badge === null) continue
        expect(badge).toHaveAttribute('role', 'img')
        expect(badge).toHaveAttribute('title', labels[slot])
        // A sign or an icon says what the number is: a sword, a minus or a skull.
        const signed = badge.querySelector('svg') !== null || /^≈?-\d/.test(badge.textContent ?? '')
        expect(signed).toBe(true)
      }
    },
  )

  it('draws the sword and the amount dealt, with ≈ when it depends on a choice', () => {
    expect(mark({ damage: 0, dies: false, dealt: 5 }).dealt).toHaveTextContent('5')
    expect(mark({ damage: 0, dies: false, dealt: 5 }).dealt?.querySelector('svg')).not.toBeNull()
    const { dealt } = mark({ damage: 0, dies: false, dealt: 5, choice: true })
    expect(dealt).toHaveTextContent('≈5')
    expect(dealt?.getAttribute('aria-label')).toContain('depending on a choice')
  })

  it('puts what a standing card deals bottom left over the stat line, and on a turned card in the top row', () => {
    const standing = mark({ damage: 1, dies: false, dealt: 3 })
    const place = standing.dealt?.parentElement
    expect(place).toHaveAttribute('data-slot', 'forecast-dealt-place')
    expect(place).toHaveClass('bottom-5', 'left-0.5')
    expect(standing.row?.contains(standing.dealt ?? null)).toBe(false)
    const turned = mark({ damage: 1, dies: false, dealt: 3 }, { tapped: true })
    expect(turned.row?.contains(turned.dealt ?? null)).toBe(true)
    expect(turned.root.querySelector('[data-slot="forecast-dealt-place"]')).toBeNull()
  })

  it('joins the skull to the damage badge on a compact card on its side, which has no room for a third row', () => {
    const { damage, doomed } = mark({ damage: 6, dies: true, dealt: 2 }, { tapped: true, compact: true })
    expect(doomed).toBeNull()
    expect(damage).toHaveAttribute('aria-label', 'Takes 6 damage and dies')
    expect(damage?.querySelector('svg')).not.toBeNull()
    expect(damage).toHaveTextContent('-6')
    // A compact card that stands keeps them apart.
    expect(mark({ damage: 6, dies: true }, { compact: true }).doomed).not.toBeNull()
  })

  it('moves the top row under a corner another mark holds', () => {
    expect(mark({ damage: 2, dies: true }).row).toHaveClass('top-0.5')
    const taken = mark({ damage: 2, dies: true }, { cornerTaken: true }).row
    expect(taken).toHaveClass('top-5.5')
    expect(taken).toHaveAttribute('data-corner-taken', 'true')
  })

  it('never takes the pointer, and the cracks are hidden from assistive technology', () => {
    const { root, cracks } = mark({ damage: 5, dies: true, dealt: 2 })
    expect(root).toHaveClass('pointer-events-none')
    expect(cracks).toHaveAttribute('aria-hidden', 'true')
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

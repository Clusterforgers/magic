// Goal: a compact turned face too short for its art row draws its marks beside the stat box in the order Attacking or Blocking, Blocked, damage, Sick, Tapped, so the first that fits is the most important; other faces draw no such row.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PermanentView } from '@/protocol/generated'
import { permanent } from '@/test-utils/cards'
import { TextFace } from './CardFace'
import { statusChips } from './cardModel'
import type { CombatRole } from './cardModel'
import { CardWidthContext } from './ImageFace'
import { turnedMarks } from './narrowMarks'

const TAPPED = { tapped: true }

const CASES: [string, Partial<PermanentView>, CombatRole | null, string[]][] = [
  ['an attacker', TAPPED, 'attacking', ['Attacking', 'Tapped']],
  ['a tapped blocker', TAPPED, 'blocking', ['Blocking', 'Tapped']],
  ['a blocked attacker', TAPPED, 'blocked', ['Blocked', 'Tapped']],
  [
    'a damaged sick attacker',
    { tapped: true, damage: 2, summoning_sick: true },
    'attacking',
    ['Attacking', '2 dmg', 'Sick', 'Tapped'],
  ],
  ['a damaged blocked attacker', { tapped: true, damage: 1 }, 'blocked', ['Blocked', '1 dmg', 'Tapped']],
  ['a sick tapped creature', { tapped: true, summoning_sick: true }, null, ['Sick', 'Tapped']],
  ['a tapped land', TAPPED, null, ['Tapped']],
  ['an exerted attacker', { tapped: true, exerted: true }, 'attacking', ['Attacking', 'Tapped']],
]

describe('turned marks', () => {
  it.each(CASES)('orders the marks of %s', (_name, state, role, labels) => {
    expect(turnedMarks(statusChips(permanent(state), role)).map((chip) => chip.label)).toEqual(labels)
  })

  it.each(CASES)(
    'draws the marks of %s in that order on a compact turned face',
    (_name, state, role, labels) => {
      const object = permanent(state, { power: 2, toughness: 3 })
      const box = render(
        <CardWidthContext.Provider value={50}>
          <TextFace object={object} role={role} size="compact" fit />
        </CardWidthContext.Provider>,
      ).container
      const row = box.querySelector('[data-turned-marks]')
      expect(row?.getAttribute('aria-hidden')).toBe('true')
      // The row lies in the art slot, whose container query shows it where the slot's own marks drop out.
      expect(row?.closest('[data-slot="card-art"]')).not.toBeNull()
      const words = [...(row?.querySelectorAll('[data-slot="card-turned-mark"]') ?? [])].map((mark) =>
        mark.getAttribute('title'),
      )
      expect(words).toEqual(
        labels.map((label) =>
          label === 'Sick' ? 'Summoning sick' : label.replace(/^(\d+) dmg$/, '$1 damage'),
        ),
      )
      expect(row?.lastElementChild?.getAttribute('data-stat')).toBe('2/3')
      expect(row?.textContent).toBe(labels.flatMap((label) => /^(\d+) dmg$/.exec(label)?.[1] ?? []).join(''))
    },
  )

  it.each(['normal', 'compact-upright'] as const)('draws no turned row on a %s face', (kind) => {
    const box = render(
      <TextFace
        object={permanent(TAPPED)}
        role="attacking"
        size={kind === 'normal' ? 'normal' : 'compact'}
        upright={kind === 'compact-upright'}
      />,
    ).container
    expect(box.querySelector('[data-turned-marks]')).toBeNull()
  })
})

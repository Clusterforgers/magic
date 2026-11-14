// Goal: a card on the table lays down on a tap and stands up on an untap with a 150 ms height change where motion is welcome, and at once under reduced motion (the change is a `motion-safe` class only).
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { TAP_MOTION } from './cardSize'

describe('the tap motion', () => {
  it('is only welcome motion, on height, 150 ms', () => {
    const classes = TAP_MOTION.split(' ')
    expect(classes.every((name) => name.startsWith('motion-safe:'))).toBe(true)
    expect(classes).toContain('motion-safe:transition-[height]')
    expect(classes).toContain('motion-safe:duration-150')
  })

  it('is on the face of every card of the battlefield, tapped or not', () => {
    const { container } = renderTable({ seats: 2 })
    const faces = [...container.querySelectorAll('[data-battlefield-seat] [data-slot="card-face"]')]
    expect(faces.length).toBeGreaterThan(10)
    expect(faces.some((face) => face.hasAttribute('data-turned'))).toBe(true)
    for (const face of faces) expect(face).toHaveClass('motion-safe:transition-[height]')
  })
})

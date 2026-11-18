// Goal: a cast or a land played leaves the hand on the press, before the server's batch, in every layout; any other answer leaves the hand alone; a refusal brings the card back; under reduced motion the card only fades.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import type { Answer } from '@/protocol/generated'
import { blockAfter } from '@/test-utils/cssBlocks'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { withBoss } from './dev/devGame'
import type { Fixture } from './dev/fixture'
import { leavingCard } from './handLeaving'
import type { Viewport } from './layout'

describe('leavingCard', () => {
  it.each<[string, Answer | null, number | null]>([
    ['nothing in flight', null, null],
    ['a cast', { Priority: { CastSpell: 7 } }, 7],
    ['a land', { Priority: { PlayLand: 8 } }, 8],
    ['a land by its back face', { Priority: { PlayLandFace: [9, 1] } }, 9],
    ['a pass', { Priority: 'Pass' }, null],
    ['an activation', { Priority: { Activate: { object: 4, ability: 0 } } }, null],
    ['a discard', { Discard: [3] }, null],
  ])('%s', (_name, answered, card) => {
    expect(leavingCard({ answered })).toBe(card)
  })
})

const slots = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-slot="hand-slot"]')]
const idOf = (slot: HTMLElement): number =>
  Number(slot.querySelector('[data-object-id]')?.getAttribute('data-object-id'))

describe('the hand on a cast', () => {
  it.each<[string, () => Fixture, Viewport]>([
    ['1v1', () => buildFixture({ seats: 2 }), { width: 1400, height: 900 }],
    ['a grid of four', () => buildFixture({ seats: 4 }), { width: 1400, height: 900 }],
    ['a boss fight', () => withBoss(buildFixture({ seats: 4 }), 0), { width: 1400, height: 900 }],
    ['a phone', () => buildFixture({ seats: 2 }), { width: 390, height: 844 }],
  ])('lets the card go at once and takes it back on a refusal at %s', (_name, fixture, viewport) => {
    const h = renderTable({ fixture: fixture(), options: { viewport } })
    const [first, second] = slots()
    if (first === undefined || second === undefined)
      throw new Error('the fixture holds a hand of two or more')
    const card = idOf(second)
    const count = slots().length
    h.update(() => ({ pending: null, answered: { Priority: { CastSpell: card } } }))
    expect(second).toHaveAttribute('data-leaving')
    expect(first).not.toHaveAttribute('data-leaving')
    // The slot keeps its room until the batch moves the card, so the hand closes up once.
    expect(slots()).toHaveLength(count)
    // The server refuses: the session clears the answer in flight, and the card stands in its place again.
    h.update(() => ({ answered: null }))
    expect(second).not.toHaveAttribute('data-leaving')
  })

  it('leaves the hand alone for a pass', () => {
    const h = renderTable({ seats: 2 })
    h.update(() => ({ pending: null, answered: { Priority: 'Pass' } }))
    expect(slots().filter((slot) => slot.hasAttribute('data-leaving'))).toEqual([])
  })
})

describe('the leave in feel.css', () => {
  const css = readFileSync(join(import.meta.dirname, '../../feel.css'), 'utf8').replace(
    /\/\*[\s\S]*?\*\//g,
    '',
  )
  // The still rules stand before the media block that holds the motion.
  const media = '@media (prefers-reduced-motion: no-preference)'
  const still = css.slice(0, css.lastIndexOf(media, css.indexOf('@keyframes feel-hand-leave')))
  const motion = blockAfter(css.slice(still.length), media)

  it('hides a leaving card and lets no pointer reach it, whatever the motion setting', () => {
    expect(blockAfter(still, "[data-slot='hand-slot'][data-leaving] > div")).toContain('opacity: 0')
    expect(blockAfter(still, "[data-slot='hand-slot'][data-leaving] {")).toContain('pointer-events: none')
  })

  it('travels only where motion is welcome: a dip, then a rise out of the slot', () => {
    expect(motion).toContain('animation: feel-hand-leave')
    const frames = blockAfter(motion, '@keyframes feel-hand-leave')
    expect(frames).toContain('scale(0.94)')
    expect(frames).toContain('translateY(-24px)')
    expect(/--hand-leave-ms:\s*(\d+)ms/.exec(css)?.[1]).toBe('180')
  })
})

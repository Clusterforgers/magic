// Goal: the marks over a card picture are small and sit at its corners — damage, counters and attachments at the foot, identity and combat marks at the head, summoning sickness as an icon and a slight dim — move with a card that lies on its side, show the real power and toughness of a token or a face-down creature, and a token without a picture still looks like a card.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice } from '@/features/cards/imageSetting'
import { indexById } from '@/play/view'
import type { CardDetail, ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'
import { setDetailFetcher } from './printed'

const ID = 900
const IDENTITY = { oracle_id: 'giant-oracle' }

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

function renderCard(object: ObjectView) {
  return renderTable({
    ui: <Card id={ID} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

/** Draws the card and lets its picture load, so the marks sit over the picture. */
async function loaded(object: ObjectView) {
  const view = renderCard(object)
  act(() => fireEvent.load(must(view.container.querySelector('img'))))
  await act(async () => {
    await Promise.resolve()
  })
  return view
}

const footOf = (container: HTMLElement) => container.querySelector('[data-slot="card-marks-foot"]')
const headOf = (container: HTMLElement) => container.querySelector('[data-slot="card-marks-head"]')
const statusTexts = (container: HTMLElement): string[] =>
  [...container.querySelectorAll('[data-slot="card-status"]')].map((chip) => chip.textContent ?? '')

const printedFaces = (power: string, toughness: string): CardDetail =>
  ({ faces: [{ name: 'Hill Giant', power, toughness, loyalty: null }] }) as unknown as CardDetail

const offline = (): void => setDetailFetcher(() => Promise.reject(new Error('offline')))

describe('the marks over a picture are small and sit at its corners', () => {
  it('draws summoning sickness as a small icon and a slight dim, never as a label', async () => {
    offline()
    const { container } = await loaded(permanent({ summoning_sick: true }, { identity: IDENTITY }))
    expect(must(headOf(container)).querySelector('[data-slot="card-sick"]')).not.toBeNull()
    expect(container.querySelector('[data-slot="card-dim"]')).not.toBeNull()
    expect(statusTexts(container)).toEqual([])
  })

  it('draws no dim for a permanent that can act', async () => {
    offline()
    const { container } = await loaded(permanent({}, { identity: IDENTITY }))
    expect(container.querySelector('[data-slot="card-dim"]')).toBeNull()
  })

  it('puts damage and counters at the foot, each as one small chip with the kind’s symbol', async () => {
    offline()
    const { container } = await loaded(
      permanent(
        { damage: 1 },
        {
          identity: IDENTITY,
          counters: [
            ['+1/+1', 2],
            ['-1/-1', 1],
            ['poison', 1],
            ['charge', 3],
            [{ Other: 'oil' }, 1],
          ],
        },
      ),
    )
    const foot = must(footOf(container))
    expect([...foot.children].map((chip) => chip.textContent)).toEqual([
      '1 dmg',
      '+1/+1 ×2',
      '−1/−1',
      '☠',
      '⚡ ×3',
      'oil',
    ])
    expect(
      [...foot.querySelectorAll('[data-slot="card-counter"]')].map((chip) => chip.getAttribute('data-tone')),
    ).toEqual(['up', 'down', 'plain', 'plain', 'plain'])
  })

  it('puts the identity marks at the head and keeps them out of the foot', async () => {
    offline()
    const { container } = await loaded(
      permanent({ prepared: true }, { identity: IDENTITY, copy: {}, face: 1 }),
    )
    expect(must(headOf(container)).textContent).toBe('Back faceCopyPrepared')
    expect(footOf(container)).toBeNull()
  })

  it('draws nothing at the corners of a plain permanent', async () => {
    offline()
    const { container } = await loaded(permanent({}, { identity: IDENTITY }))
    expect(footOf(container)).toBeNull()
    expect(headOf(container)).toBeNull()
  })

  it('moves the corners with a card that lies on its side', async () => {
    offline()
    const { container } = await loaded(
      permanent({ tapped: true, damage: 1 }, { identity: IDENTITY, copy: {} }),
    )
    expect(must(footOf(container)).className).toContain('top-[3%]')
    expect(must(headOf(container)).className).toContain('bottom-[3%]')
  })
})

describe('real power and toughness', () => {
  const token = (over: Partial<ObjectView> = {}): ObjectView =>
    permanent(
      {},
      {
        kind: 'Token',
        name: 'Thopter',
        types: ['Creature'],
        subtypes: ['Thopter'],
        power: 2,
        toughness: 2,
        ...over,
      },
    )

  it('shows the numbers of a token plainly, since its picture has nothing to compare with', async () => {
    const { container } = await loaded(token())
    const badge = must(container.querySelector('[data-slot="card-pt"]'))
    expect(badge).toHaveTextContent('2/2')
    expect(badge).not.toHaveAttribute('data-modified')
  })

  it('shows the 2/2 of a face-down creature plainly, even where the card behind it is a 5/5', async () => {
    setDetailFetcher(() => Promise.resolve(printedFaces('5', '5')))
    const { container } = await loaded(
      permanent({}, { identity: IDENTITY, face_down: true, power: 2, toughness: 2 }),
    )
    const badge = must(container.querySelector('[data-slot="card-pt"]'))
    expect(badge).toHaveTextContent('2/2')
    expect(badge).not.toHaveAttribute('data-modified')
  })

  it('shows what a face-down creature of another seat carries on its back', () => {
    const { container } = renderCard(
      permanent(
        { damage: 1 },
        {
          face_down: true,
          name: '',
          types: ['Creature'],
          subtypes: [],
          colors: [],
          power: 2,
          toughness: 2,
          counters: [['+1/+1', 1]],
        },
      ),
    )
    expect(screen.getByRole('img', { name: 'Face-down card' })).toBeInTheDocument()
    expect(must(container.querySelector('[data-slot="card-pt"]'))).toHaveTextContent('2/2')
    expect(must(footOf(container)).textContent).toBe('1 dmg+1/+1')
    expect(statusTexts(container)).toEqual(['1 dmg', 'Face down'])
  })

  it('draws a token whose picture is missing as a card: the back’s colours, its name, type and numbers', () => {
    const { container } = renderCard(token())
    const face = must(container.querySelector('[data-slot="card-face"]'))
    expect(face.className).toContain('bg-card-back')
    expect(face).toHaveTextContent('Thopter')
    expect(must(container.querySelector('[data-slot="card-type-line"]'))).toHaveTextContent(
      'Token Creature — Thopter',
    )
    expect(face).toHaveTextContent('2/2')
  })

  it('draws a card whose picture is missing in its own colour, not the back’s', () => {
    const { container } = renderCard(permanent({}, { identity: IDENTITY }))
    expect(must(container.querySelector('[data-slot="card-face"]')).className).not.toContain('bg-card-back')
  })
})

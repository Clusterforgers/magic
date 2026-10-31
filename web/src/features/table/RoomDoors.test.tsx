// Goal: a Room permanent shows its doors as a badge of two marks (the half's name, a lock that is open once the door is unlocked), on its picture and on its text face, the icons alone when compact with the words kept for a screen reader; the text face titles a Room with both doors locked by its doors, the spoken name tells each door and its state, other permanents carry no badge, and the preview lists both halves with their state and rules text and offers no face to turn over.
import { act, fireEvent, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice, setImageChoice } from '@/features/cards/imageSetting'
import type { CardsApi } from '@/net/cards'
import { indexById } from '@/play/view'
import type { CardDetail, CardFaceView, ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { permanent } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { PreviewCard } from './PreviewCard'
import { setDetailFetcher } from './printed'

const ROOM = 910
const ENTRYWAY = 'Grand Entryway'
const ROTUNDA = 'Elegant Rotunda'
const IDENTITY = { oracle_id: 'room-oracle' }

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

const face = (name: string, oracle_text: string): CardFaceView => ({
  name,
  mana_cost: '',
  type_line: 'Enchantment — Room',
  oracle_text,
  power: null,
  toughness: null,
  loyalty: null,
  defense: null,
  colors: [],
})

const FACES = [
  face(ENTRYWAY, 'When you unlock this door, create a 1/1 white Glimmer creature enchantment token.'),
  face(ROTUNDA, 'Flying'),
]

/** What the server's card endpoint answers for the Room. */
const api = (): CardsApi => ({
  search: () => Promise.reject(new Error('unused')),
  get: () => Promise.resolve({ faces: FACES } as unknown as CardDetail),
})

/** A Room permanent with these doors unlocked and the name the server gives it (none locked, the door, or the whole card). */
function room(doors: [boolean, boolean], over: Partial<ObjectView> = {}): ObjectView {
  const [first, second] = doors
  const name = first && second ? `${ENTRYWAY} // ${ROTUNDA}` : first ? ENTRYWAY : second ? ROTUNDA : ''
  return permanent(
    { doors },
    {
      id: ROOM,
      name,
      mana_cost: null,
      types: ['Enchantment'],
      subtypes: ['Room'],
      colors: ['White'],
      power: null,
      toughness: null,
      identity: IDENTITY,
      ...over,
    },
  )
}

function renderRoom(object: ObjectView, size: CardSize = 'normal') {
  return renderTable({
    ui: <Card id={object.id} size={size} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, object]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

/** Lets the faces of the card arrive. */
const arrived = () =>
  act(async () => {
    await Promise.resolve()
  })

const doorsOf = (container: HTMLElement) =>
  [...container.querySelectorAll<HTMLElement>('[data-slot="card-door"]')].map((mark) => ({
    door: mark.dataset['door'],
    unlocked: mark.dataset['unlocked'],
    text: mark.textContent,
    icon: mark.querySelector('svg')?.getAttribute('class') ?? '',
  }))

describe('the door badge on a text face', () => {
  it.each<[string, [boolean, boolean]]>([
    ['both doors locked', [false, false]],
    ['the first door unlocked', [true, false]],
    ['the second door unlocked', [false, true]],
    ['both doors unlocked', [true, true]],
  ])('names both halves and their state with %s', async (_case, doors) => {
    setImageChoice('text')
    setDetailFetcher(() => Promise.resolve({ faces: FACES } as unknown as CardDetail))
    const { container } = renderRoom(room(doors))
    await arrived()
    const marks = doorsOf(container)
    expect(marks.map((mark) => mark.door)).toEqual(['0', '1'])
    expect(marks.map((mark) => mark.unlocked)).toEqual(doors.map(String))
    expect(marks.map((mark) => mark.text)).toEqual([
      `${ENTRYWAY}${doors[0] ? ', unlocked' : ', locked'}`,
      `${ROTUNDA}${doors[1] ? ', unlocked' : ', locked'}`,
    ])
  })

  it('titles a Room with both doors locked by its doors, the name it has none of', async () => {
    setImageChoice('text')
    setDetailFetcher(() => Promise.resolve({ faces: FACES } as unknown as CardDetail))
    const { container } = renderRoom(room([false, false]))
    await arrived()
    expect(must(container.querySelector('[data-slot="card-face"]'))).toHaveTextContent(
      `${ENTRYWAY} // ${ROTUNDA}`,
    )
  })

  it('says "Room" and the places of the doors while the faces are unknown', () => {
    setImageChoice('text')
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const { container } = renderRoom(room([false, false]))
    expect(must(container.querySelector('[data-slot="card-face"]'))).toHaveTextContent('Room')
    expect(doorsOf(container).map((mark) => mark.text)).toEqual(['First door, locked', 'Second door, locked'])
  })

  it('shows an open lock for an unlocked door and a closed one for a locked door', async () => {
    setImageChoice('text')
    setDetailFetcher(() => Promise.resolve({ faces: FACES } as unknown as CardDetail))
    const { container } = renderRoom(room([true, false]))
    await arrived()
    const [open, shut] = doorsOf(container)
    expect(open?.icon).toContain('lucide-lock-open')
    expect(shut?.icon).toContain('lucide-lock')
    expect(shut?.icon).not.toContain('lucide-lock-open')
  })

  it('keeps the words for a screen reader when compact', async () => {
    setImageChoice('text')
    setDetailFetcher(() => Promise.resolve({ faces: FACES } as unknown as CardDetail))
    const { container } = renderRoom(room([true, false]), 'compact')
    await arrived()
    for (const mark of container.querySelectorAll('[data-slot="card-door"]')) {
      expect(must(mark.querySelector('span'))).toHaveClass('sr-only')
    }
    expect(doorsOf(container).map((mark) => mark.text)).toEqual([
      `${ENTRYWAY}, unlocked`,
      `${ROTUNDA}, locked`,
    ])
  })

  it('is left out of a permanent that is not a Room', () => {
    setImageChoice('text')
    const { container } = renderRoom(permanent({}, { id: ROOM }))
    expect(container.querySelector('[data-slot="card-door"]')).toBeNull()
  })

  it('is in the spoken name of the card, door by door', async () => {
    setImageChoice('text')
    setDetailFetcher(() => Promise.resolve({ faces: FACES } as unknown as CardDetail))
    const { container } = renderRoom(room([true, false]))
    await arrived()
    expect(must(container.querySelector('button')).getAttribute('aria-label')).toContain(
      `Room, ${ENTRYWAY} unlocked, ${ROTUNDA} locked`,
    )
  })
})

describe('the door badge on a picture', () => {
  it('sits in the foot stack once the picture has loaded', async () => {
    setDetailFetcher(() => Promise.resolve({ faces: FACES } as unknown as CardDetail))
    const { container } = renderRoom(room([false, true]))
    act(() => fireEvent.load(must(container.querySelector('img'))))
    await arrived()
    const foot = must(container.querySelector('[data-slot="card-marks-foot"]'))
    expect([...foot.querySelectorAll('[data-slot="card-door"]')].map((mark) => mark.textContent)).toEqual([
      `${ENTRYWAY}, locked`,
      `${ROTUNDA}, unlocked`,
    ])
  })
})

describe('the preview of a Room', () => {
  async function drawn(doors: [boolean, boolean]) {
    const view = renderTable({
      ui: <PreviewCard object={room(doors)} role={null} size="large" api={api()} />,
    })
    await arrived()
    return view
  }

  it.each<[string, [boolean, boolean]]>([
    ['both locked', [false, false]],
    ['one unlocked', [true, false]],
    ['both unlocked', [true, true]],
  ])('lists both halves with their state and rules text, %s', async (_case, doors) => {
    const { container } = await drawn(doors)
    const items = [...container.querySelectorAll('[data-slot="preview-doors"] > li')]
    expect(items.map((item) => item.getAttribute('data-unlocked'))).toEqual(doors.map(String))
    expect(items.map((item) => item.querySelector('p')?.textContent)).toEqual([
      `${ENTRYWAY} ${doors[0] ? 'unlocked' : 'locked'}`,
      `${ROTUNDA} ${doors[1] ? 'unlocked' : 'locked'}`,
    ])
    expect(screen.getByText('Flying')).toHaveClass('select-text')
    expect(screen.getByText(/create a 1\/1 white Glimmer/)).toBeInTheDocument()
  })

  it('offers no face to turn over', async () => {
    await drawn([true, false])
    expect(screen.queryByRole('button', { name: /^Show / })).toBeNull()
  })
})

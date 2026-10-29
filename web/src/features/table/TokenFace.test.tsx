// Goal: a token without a picture is drawn as a card (name plate, type line, rules box, boxed power/toughness in its colour frame), the miss is remembered for the session whatever its power and toughness become, and a token that copies a card asks for the card's picture.
import { act, fireEvent } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice } from '@/features/cards/imageSetting'
import type { ObjectView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { indexById } from '@/play/view'
import { Card } from './Card'

const ID = 901

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

/** A Thopter token as Chief of the Foundry leaves it. */
const thopter = (power: number, over: Partial<ObjectView> = {}): ObjectView =>
  card({
    name: 'Thopter',
    kind: 'Token',
    mana_cost: null,
    types: ['Artifact', 'Creature'],
    subtypes: ['Thopter'],
    colors: [],
    power,
    toughness: power,
    keywords: ['Flying'],
    ...over,
  })

function renderToken(object: ObjectView) {
  return renderTable({
    ui: <Card id={ID} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

const urlOf = (container: HTMLElement) => must(container.querySelector('img')).getAttribute('src')
const face = (container: HTMLElement) => container.querySelector('[data-slot="card-face"]')

/** Draws the token and has its picture come back missing. */
function missing(object: ObjectView) {
  const view = renderToken(object)
  act(() => fireEvent.error(must(view.container.querySelector('img'))))
  return view
}

describe('a token whose picture is missing', () => {
  it('asks for the picture by the facts it shows, power and toughness included', () => {
    const { container } = renderToken(thopter(2))
    expect(urlOf(container)).toBe(
      '/api/images/token/0/small?name=Thopter&type=Artifact+Creature+%E2%80%94+Thopter&colors=C&power=2&toughness=2',
    )
  })

  it('shows the plain text face while the picture loads', () => {
    const { container } = renderToken(thopter(2))
    expect(face(container)).not.toBeNull()
    expect(face(container)).not.toHaveAttribute('data-card-like')
  })

  it('is drawn as a card: name, type line, keywords and the power/toughness box in its frame', () => {
    const { container } = missing(thopter(2))
    const drawn = must(face(container))
    expect(drawn).toHaveAttribute('data-card-like', 'true')
    expect(drawn).toHaveAttribute('data-frame', 'colorless')
    expect(drawn).toHaveTextContent('Thopter')
    expect(must(drawn.querySelector('[data-slot="card-type"]'))).toHaveTextContent(
      'Token Artifact Creature — Thopter',
    )
    expect(must(drawn.querySelector('[data-slot="card-keywords"]'))).toHaveTextContent('Flying')
    expect(drawn).toHaveTextContent('2/2')
    expect(container.querySelector('img')).toBeNull()
  })

  it('takes the frame of its colour', () => {
    const { container } = missing(thopter(1, { colors: ['White'] }))
    expect(face(container)).toHaveAttribute('data-frame', 'white')
  })

  it('is not asked for again when it comes back with other power and toughness', () => {
    missing(thopter(2)).unmount()
    const { container } = renderToken(thopter(3))
    expect(container.querySelector('img')).toBeNull()
    expect(face(container)).toHaveAttribute('data-card-like', 'true')
    expect(face(container)).toHaveTextContent('3/3')
  })

  it('stays asked for under another name', () => {
    missing(thopter(2)).unmount()
    const { container } = renderToken(thopter(2, { name: 'Servo', subtypes: ['Servo'] }))
    expect(urlOf(container)).toContain('name=Servo')
  })
})

describe('a printed card whose picture is missing', () => {
  it('keeps the plain text face', () => {
    const { container } = missing(card({ identity: { oracle_id: 'bears-oracle' } }))
    expect(face(container)).not.toBeNull()
    expect(face(container)).not.toHaveAttribute('data-card-like')
  })
})

describe('a token that copies a card', () => {
  it('asks for the card picture, not a token picture', () => {
    const copy = thopter(1, {
      name: 'Grizzly Bears',
      copy: { of: 5 },
      identity: { oracle_id: 'bears-oracle' },
    })
    const { container } = renderToken(copy)
    expect(urlOf(container)).toBe('/api/images/oracle/bears-oracle/0/small')
  })
})

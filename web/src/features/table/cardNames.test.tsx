// Goal: a text face writes its name word by word, so a name breaks only between words and a word too wide for the face ends in an ellipsis instead of splitting.
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice } from '@/features/cards/imageSetting'
import { indexById } from '@/play/view'
import type { ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from './Card'

const ID = 901

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

function renderNamed(name: string) {
  const object: ObjectView = { ...card({ name }), id: ID }
  return renderTable({
    ui: <Card id={ID} />,
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, object]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

describe('the name of a text face', () => {
  it.each([
    ['Mountain', ['Mountain']],
    ['Lightning Bolt', ['Lightning', 'Bolt']],
    ['Thrasios, Triton Hero', ['Thrasios,', 'Triton', 'Hero']],
  ])('%s is %j, each word cut by an ellipsis rather than by a break', (name, words) => {
    const view = renderNamed(name)
    const spans = Array.from(view.container.querySelectorAll('[data-name] .truncate'))
    expect(spans.map((span) => span.textContent)).toEqual(words)
    expect(view.container.querySelector('[data-name]')?.textContent).toBe(words.join(' '))
  })
})

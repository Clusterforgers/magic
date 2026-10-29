// Goal: a picture stays hidden behind its text face until it loads, a missing one (error, 404, 502) leaves the text face and never a broken image, state is drawn only over a loaded picture, and the Text-only choice persists per viewer.
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { CardBack, CardImage, RETRY_AFTER_MS, knownStatus, resetImageMemory } from './CardImage'
import { IMAGE_CHOICE_KEY, imageChoice, resetImageChoice, setImageChoice } from './imageSetting'

// A fixture URL: tests never reach the network.
const SRC = '/api/images/oracle/test/0/small'

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

function draw(src: string | null = SRC) {
  return render(
    <CardImage src={src} alt="Grizzly Bears" overlay={<span>2/2 badge</span>}>
      <span>text face</span>
    </CardImage>,
  )
}

describe('CardImage', () => {
  it.each<['load' | 'error', string, boolean, boolean]>([
    ['load', 'loaded', false, true],
    ['error', 'failed', true, false],
  ])('on %s it is %s', (event, status, textFace, picture) => {
    const { container } = draw()
    const img = screen.getByRole('img', { hidden: true })
    expect(img).toHaveAttribute('loading', 'lazy')
    expect(img).toHaveAttribute('decoding', 'async')
    expect(screen.getByText('text face')).toBeInTheDocument()
    expect(screen.queryByText('2/2 badge')).toBeNull()
    act(() => {
      if (event === 'load') fireEvent.load(img)
      else fireEvent.error(img)
    })
    expect(container.querySelector('[data-slot="card-image"]')).toHaveAttribute('data-status', status)
    expect(screen.queryByText('text face') !== null).toBe(textFace)
    expect(container.querySelector('img') !== null).toBe(picture)
    expect(screen.queryByText('2/2 badge') !== null).toBe(picture)
  })

  it('names a loaded picture after the card', () => {
    draw()
    act(() => fireEvent.load(screen.getByRole('img', { hidden: true })))
    expect(screen.getByRole('img', { name: 'Grizzly Bears' })).toBeInTheDocument()
  })

  it('draws the text face alone without a source', () => {
    const { container } = draw(null)
    expect(container.querySelector('img')).toBeNull()
    expect(screen.getByText('text face')).toBeInTheDocument()
  })

  it('remembers a missing picture for a while, then asks again', () => {
    draw()
    act(() => fireEvent.error(screen.getByRole('img', { hidden: true })))
    const now = Date.now()
    expect(knownStatus(SRC, now)).toBe('failed')
    expect(knownStatus(SRC, now + RETRY_AFTER_MS + 1)).toBe('loading')
  })

  it.each<['load' | 'error', string, boolean]>([
    ['load', 'loaded', false],
    ['error', 'failed', true],
  ])('asks for the fallback once the printing is missing; on %s it is %s', (event, status, textFace) => {
    const FALLBACK = '/api/images/oracle/card/0/small'
    const { container } = render(
      <CardImage src="/api/images/printing/p/0/small" fallback={FALLBACK} alt="Grizzly Bears">
        <span>text face</span>
      </CardImage>,
    )
    act(() => fireEvent.error(screen.getByRole('img', { hidden: true })))
    const img = screen.getByRole('img', { hidden: true })
    expect(img).toHaveAttribute('src', FALLBACK)
    act(() => {
      if (event === 'load') fireEvent.load(img)
      else fireEvent.error(img)
    })
    expect(container.querySelector('[data-slot="card-image"]')).toHaveAttribute('data-status', status)
    expect(screen.queryByText('text face') !== null).toBe(textFace)
  })

  it('draws a neutral back with a name when given one', () => {
    render(<CardBack label="Face-down card" />)
    expect(screen.getByRole('img', { name: 'Face-down card' })).toBeInTheDocument()
  })
})

describe('the Card images choice', () => {
  it.each<[string | null, 'on' | 'text']>([
    [null, 'on'],
    ['on', 'on'],
    ['text', 'text'],
    ['junk', 'on'],
  ])('stored %s reads %s', (stored, choice) => {
    if (stored !== null) window.localStorage.setItem(IMAGE_CHOICE_KEY, stored)
    expect(imageChoice()).toBe(choice)
  })

  it('stores a change', () => {
    setImageChoice('text')
    expect(window.localStorage.getItem(IMAGE_CHOICE_KEY)).toBe('text')
    resetImageChoice()
    expect(imageChoice()).toBe('text')
  })

  it('survives storage that throws', () => {
    const spy = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(imageChoice()).toBe('on')
    spy.mockRestore()
  })
})

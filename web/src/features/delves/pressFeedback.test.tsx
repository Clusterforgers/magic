// Goal: a Delves vote and a shop buy answer the press in its frame: the chosen card beats (where motion is welcome), a vote on its way stands marked as the vote, whole while the other camps dim, and a camp's vote count and the purse pop as they change (never on their first draw).
import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { DelveAction, DelveView, Offer, Run } from '@/protocol/generated'
import { BEAT_MS } from './beat'
import { DelveOffer } from './DelveOffer'
import { CONTENT, delveView, hero } from './fakeDelves'
import { VotePanel } from './VotePanel'

const calls: { element: Element; keyframes: Keyframe[]; options: KeyframeAnimationOptions }[] = []

function motion(reduce: boolean): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: reduce && query.includes('reduce'),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

beforeEach(() => {
  calls.length = 0
  motion(false)
  Element.prototype.animate = function (
    this: Element,
    keyframes: Keyframe[],
    options: KeyframeAnimationOptions,
  ) {
    calls.push({ element: this, keyframes, options })
    return {} as Animation
  } as Element['animate']
})
afterEach(() => {
  vi.unstubAllGlobals()
  Reflect.deleteProperty(Element.prototype, 'animate')
})

const ana = hero()
const bo = hero({ name: 'Bo', user: 'u1' })
const map = (votes: Run['votes']): DelveView =>
  delveView({}, { status: 'Map', at: 0, cleared: [0], votes, heroes: [ana, bo] })

describe('a vote', () => {
  it('beats the chosen camp in the frame of the press and marks it as the vote while it is on its way', () => {
    const act = vi.fn<(action: DelveAction) => void>()
    const { rerender } = render(<VotePanel view={map({})} act={act} busy={false} />)
    const event = screen.getByRole('button', { name: 'Vote for Event' })
    fireEvent.click(event)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.element).toBe(event)
    expect(calls[0]?.options.duration).toBe(BEAT_MS)
    expect(String(calls[0]?.keyframes[1]?.transform)).toBe('scale(0.96)')
    // The page marks itself busy for the answer: the vote stands at once, whole, and the other camp dims.
    rerender(<VotePanel view={map({})} act={act} busy />)
    const chosen = screen.getByRole('button', { name: 'Your vote: Event' })
    expect(chosen).toHaveAttribute('aria-pressed', 'true')
    expect(chosen).not.toHaveClass('disabled:opacity-70')
    expect(screen.getByRole('button', { name: 'Vote for Shop' })).toHaveClass('disabled:opacity-70')
    // The answer: the run says where the vote stands.
    rerender(<VotePanel view={map({ 0: 2 })} act={act} busy={false} />)
    expect(screen.getByRole('button', { name: 'Your vote: Event' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('pops a camp’s count as it changes, and not on its first draw', () => {
    const act = vi.fn<(action: DelveAction) => void>()
    const { rerender } = render(<VotePanel view={map({})} act={act} busy={false} />)
    const counts = () => [...document.querySelectorAll<HTMLElement>('[data-slot="ticking-count"]')]
    expect(counts().map((count) => count.hasAttribute('data-tick'))).toEqual([false, false])
    rerender(<VotePanel view={map({ 1: 2 })} act={act} busy={false} />)
    const popped = counts().filter((count) => count.hasAttribute('data-tick'))
    expect(popped).toHaveLength(1)
    expect(popped[0]).toHaveTextContent('1')
    expect(popped[0]).toHaveClass('motion-safe:animate-in', 'motion-safe:zoom-in-150')
  })

  it('does not move under reduced motion', () => {
    motion(true)
    render(<VotePanel view={map({})} act={vi.fn()} busy={false} />)
    fireEvent.click(screen.getByRole('button', { name: 'Vote for Event' }))
    expect(calls).toEqual([])
  })
})

describe('a shop buy', () => {
  const shop: Offer = { Shop: { items: ['Draught', { Card: { name: 'Sol Ring', sold: false } }] } }

  it('beats the offer bought, sends the buy, and pops the purse as it is spent', () => {
    const onPick = vi.fn()
    const { rerender } = render(
      <DelveOffer
        offer={shop}
        gold={100}
        content={CONTENT}
        busy={false}
        find={async () => null}
        onPick={onPick}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /^Healing draught/ }))
    expect(onPick).toHaveBeenCalledWith({ Take: 0 })
    expect(calls[0]?.element).toBe(document.querySelector('[data-slot="shop-offer"]'))
    const purse = () => document.querySelector<HTMLElement>('[data-slot="ticking-count"]')
    expect(purse()).not.toHaveAttribute('data-tick')
    rerender(
      <DelveOffer
        offer={shop}
        gold={70}
        content={CONTENT}
        busy={false}
        find={async () => null}
        onPick={onPick}
      />,
    )
    expect(purse()).toHaveTextContent('70')
    expect(purse()).toHaveAttribute('data-tick')
  })
})

// Goal: the marker beside a deck's name says the badge's word for a clean saved check, a deck played by hand, a blocked deck and one never checked, and "Changed since the last check" for a draft that differs from the saved deck or a stored check that is stale, whatever state the saved deck had, so a Verified marker never outlives the deck it was earned by.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DeckState } from '@/protocol/generated'
import { DeckMarker, MARKER_LABEL, markerKind } from './DeckMarker'
import type { MarkerChange, MarkerKind } from './DeckMarker'

const CASES: ReadonlyArray<readonly [string, DeckState, MarkerChange, MarkerKind, string]> = [
  ['a clean saved check', 'verified', {}, 'verified', 'Verified'],
  ['a deck played by hand', 'manual_play', {}, 'manual_play', 'Played by hand'],
  ['a blocked deck', 'blocked', {}, 'blocked', 'Blocked'],
  ['a deck never checked', 'unchecked', { changed: false }, 'unchecked', 'Unchecked'],
  ['a stale stored check', 'unchecked', { changed: true }, 'changed', 'Changed since the last check'],
  ['a draft of a verified deck', 'verified', { dirty: true }, 'changed', 'Changed since the last check'],
  [
    'a draft of a deck played by hand',
    'manual_play',
    { dirty: true },
    'changed',
    'Changed since the last check',
  ],
  ['a draft of a blocked deck', 'blocked', { dirty: true }, 'changed', 'Changed since the last check'],
  ['a draft of an unchecked deck', 'unchecked', { dirty: true }, 'changed', 'Changed since the last check'],
]

describe('markerKind', () => {
  it.each(CASES)('for %s gives the marker %s', (_what, state, change, kind) => {
    expect(markerKind(state, change)).toBe(kind)
  })

  it('shows the saved state when nothing changed', () => {
    expect(markerKind('verified')).toBe('verified')
  })
})

describe('DeckMarker', () => {
  it.each(CASES)('for %s says the word of the marker', (_what, state, change, kind, word) => {
    const { container } = render(<DeckMarker state={state} {...change} />)
    const found = container.querySelector('[data-marker]')
    expect(found).toHaveAttribute('data-marker', kind)
    expect(found).toHaveTextContent(new RegExp(`^${word}$`))
    expect(found?.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
    expect(MARKER_LABEL[kind]).toBe(word)
  })

  it('gives each of the five markers its own icon, so colour never carries the meaning alone', () => {
    const icons = ['verified', 'manual_play', 'blocked', 'unchecked', 'changed'].map((kind) => {
      const state: DeckState = kind === 'changed' ? 'verified' : (kind as DeckState)
      const { container } = render(<DeckMarker state={state} dirty={kind === 'changed'} />)
      return container.querySelector('[data-marker] svg')?.getAttribute('class')
    })
    expect(icons.every((icon) => (icon ?? '').includes('lucide-'))).toBe(true)
    expect(new Set(icons).size).toBe(5)
  })

  it.each<[string, DeckState, MarkerChange, string | null]>([
    ['a blocked deck', 'blocked', {}, 'Cannot be taken to a game until its issues are fixed'],
    ['a verified deck', 'verified', {}, null],
    ['a deck played by hand', 'manual_play', {}, null],
    ['a deck never checked', 'unchecked', { changed: false }, null],
    ['a draft of a blocked deck', 'blocked', { dirty: true }, null],
  ])('describes what %s means in the lobby’s words', (_what, state, change, meaning) => {
    const { container } = render(<DeckMarker state={state} {...change} />)
    const found = container.querySelector('[data-marker]')
    if (meaning === null) expect(found).not.toHaveAttribute('title')
    else expect(found).toHaveAttribute('title', meaning)
  })

  it('takes a class for placement', () => {
    const { container } = render(<DeckMarker state="verified" className="ml-2" />)
    expect(container.querySelector('[data-marker]')).toHaveClass('ml-2')
  })
})

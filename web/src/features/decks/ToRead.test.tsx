// Goal: where a deck check says drafts wait for a player, "N cards to read" appears as a link to the review screen (a deck by id or a precon by code, none for a deck that is not saved), nothing is drawn for none, and a draft note speaks only for a draft that waits or was found wrong.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ReadTarget } from '@/lib/toRead'
import type { ReadState } from '@/protocol/generated'
import { DraftNote } from './DraftNote'
import { renderInRouter } from '@/test-utils/router'
import { ToRead } from './ToRead'

describe('ToRead', () => {
  it.each<[number, ReadTarget | undefined, string, string | null]>([
    [1, { deck: 'deck-1' }, '1 card to read', '/read?deck=deck-1'],
    [12, { precon: 'c18-adaptive-enchantment' }, '12 cards to read', '/read?precon=c18-adaptive-enchantment'],
    [3, undefined, '3 cards to read', null],
  ])('for %i cards says "%s" as a link to the review screen at %s', async (count, target, words, path) => {
    await renderInRouter(<ToRead count={count} target={target} />)
    const found = document.querySelector('[data-slot="to-read"]')
    expect(found).toHaveTextContent(new RegExp(`^${words}$`))
    if (path === null) {
      expect(found).not.toHaveAttribute('data-read-path')
      expect(found?.tagName).toBe('SPAN')
    } else {
      expect(found).toHaveAttribute('data-read-path', path)
      expect(found).toHaveAttribute('href', path)
    }
  })

  it('draws nothing for no cards', () => {
    const { container } = render(<ToRead count={0} target={{ deck: 'deck-1' }} />)
    expect(container).toBeEmptyDOMElement()
  })
})

describe('DraftNote', () => {
  it.each<[ReadState | undefined, string | null]>([
    ['pending', 'A draft of its rules waits for a player to read it'],
    ['wrong', 'A player found the draft of its rules wrong, so the card stays played by hand'],
    ['approved', null],
    [undefined, null],
  ])('for a draft that is %s says %s', (draft, words) => {
    const { container } = render(<DraftNote draft={draft} />)
    const found = container.querySelector('[data-slot="draft-note"]')
    if (words === null) expect(found).toBeNull()
    else expect(found).toHaveTextContent(words)
  })
})

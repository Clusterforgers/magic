// Goal: the builder's deck check tells a deck whose cards are Verified or approved reads from one with drafts waiting: the marker says "Runs on reads", the counts line names the copies on reads, the check bar says "N cards to read" with the saved deck's review address (once, however many zones hold the card), and the cards played by hand say whether their draft waits or was found wrong.
import { describe, expect, it } from 'vitest'
import { checked } from './checkedDeck'
import type { Entry } from './checkedDeck'
import { BOLT, deckPane, DURESS, GROWTH, MOUNTAIN, renderBuilder, SOL_RING } from './renderBuilder'

const toRead = (): Element | null => deckPane().querySelector('[data-slot="check-to-read"]')
const counts = (): Element | null => deckPane().querySelector('[data-slot="check-counts"]')

const ON_READS: Entry[] = [
  { card: MOUNTAIN, count: 24, support: 'verified' },
  { card: BOLT, count: 4, support: 'read', draft: 'approved' },
]

const WAITING: Entry[] = [
  { card: MOUNTAIN, count: 20, support: 'verified' },
  { card: BOLT, count: 4, support: 'read', draft: 'approved' },
  { card: GROWTH, count: 2, support: 'manual', draft: 'pending' },
  { card: GROWTH, count: 1, support: 'manual', draft: 'pending', zone: 'sideboard' },
  { card: SOL_RING, count: 1, support: 'manual', draft: 'wrong' },
  { card: DURESS, count: 2, support: 'manual' },
]

describe('a deck that runs on reads', () => {
  it('has the marker "Runs on reads" and counts its reads beside the verified cards', async () => {
    await renderBuilder({ view: checked(ON_READS) })
    expect(deckPane().querySelector('[data-marker]')).toHaveAttribute('data-marker', 'read')
    expect(deckPane().querySelector('[data-marker]')).toHaveTextContent(/^Runs on reads$/)
    expect(counts()).toHaveTextContent(/^24 of 28 verified, 4 on reads$/)
    expect(deckPane().querySelector('[data-slot="hand-summary"]')).toHaveTextContent('The whole deck runs.')
    expect(toRead()).toBeNull()
  })
})

describe('a deck with drafts waiting for a read', () => {
  it('stays "Played by hand" and says how many cards wait, once for a card in two zones, with the address of the deck', async () => {
    await renderBuilder({ view: checked(WAITING) })
    expect(deckPane().querySelector('[data-marker]')).toHaveTextContent(/^Played by hand$/)
    expect(counts()).toHaveTextContent(/^20 of 30 verified, 4 on reads, 6 played by hand$/)
    expect(toRead()).toHaveTextContent(/^1 card to read$/)
    expect(toRead()?.querySelector('[data-slot="to-read"]')).toHaveAttribute(
      'data-read-path',
      '/read?deck=deck-1',
    )
  })

  it('says on each card played by hand whether its draft waits for a read or was found wrong', async () => {
    await renderBuilder({ view: checked(WAITING) })
    const entry = (id: string): Element | null => deckPane().querySelector(`li[data-hand="${id}"]`)
    expect(entry(GROWTH.oracle_id)?.querySelector('[data-draft="pending"]')).toHaveTextContent(
      'A draft of its rules waits for a player to read it',
    )
    expect(entry(SOL_RING.oracle_id)?.querySelector('[data-draft="wrong"]')).not.toBeNull()
    expect(entry(DURESS.oracle_id)?.querySelector('[data-slot="draft-note"]')).toBeNull()
  })

  it('draws no count for a deck with none waiting', async () => {
    await renderBuilder({ view: checked([{ card: MOUNTAIN, count: 20, support: 'verified' }]) })
    expect(toRead()).toBeNull()
  })
})

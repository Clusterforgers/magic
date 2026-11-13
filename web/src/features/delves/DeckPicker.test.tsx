// Goal: the Delves deck picker shows the decks as cards in collapsible groups (the hero's deck's group open first), narrows them by search and color, previews the deck picked, sends only on Take and remembers the group of the deck taken.
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { lastGroup } from '@/features/decks/groups/lastGroup'
import { DeckPicker } from './DeckPicker'
import { PICKER_ENTRIES, PICKER_GROUPS } from './fakePickerDecks'

afterEach(() => localStorage.clear())

const NOW = Date.parse('2026-11-13T12:00:00Z')

function picker(current: string | null = null, onTake = vi.fn()) {
  render(
    <DeckPicker
      heroName="Ana"
      entries={PICKER_ENTRIES}
      groups={PICKER_GROUPS}
      current={current}
      played={{ 'd-krenko': '2026-11-10T12:00:00Z' }}
      now={NOW}
      onTake={onTake}
      onCancel={() => undefined}
    />,
  )
  return onTake
}

const cards = () =>
  screen
    .queryAllByRole('button', { pressed: false })
    .concat(screen.queryAllByRole('button', { pressed: true }))
    .map((b) => b.getAttribute('data-deck'))
    .filter((id) => id !== null)
    .sort()

describe('DeckPicker', () => {
  it.each([
    [null, 'fun'],
    ['d-meren', 'hard'],
    ['d-atraxa', 'ungrouped'],
    ['precon:x', 'precons'],
  ])('with the deck %s opens the group %s first', (current, open) => {
    picker(current)
    const expanded = screen
      .getAllByRole('button', { expanded: true })
      .map((b) => b.closest('[data-group]')?.getAttribute('data-group'))
    expect(expanded).toEqual([open])
  })

  it.each([
    ['krenko', [], ['d-krenko']],
    ['', ['G'], ['d-atraxa', 'd-meren', 'precon:x']],
    ['', ['R', 'W'], ['precon:ltc']],
    ['dragon', ['U'], ['precon:cmm']],
  ])('searching %j with colors %j shows every match, groups open', (query, colours, expected) => {
    picker()
    fireEvent.change(screen.getByRole('searchbox', { name: 'Search decks' }), { target: { value: query } })
    for (const letter of colours) {
      const name = { W: 'White', U: 'Blue', R: 'Red', G: 'Green' }[letter] ?? letter
      fireEvent.click(screen.getByRole('button', { name }))
    }
    expect(cards()).toEqual(expected)
  })

  it('says when nothing matches', () => {
    picker()
    fireEvent.change(screen.getByRole('searchbox', { name: 'Search decks' }), { target: { value: 'zzz' } })
    expect(screen.getByText('No deck matches. Clear the search or a color.')).toBeInTheDocument()
  })

  it('opens and closes a group', () => {
    picker()
    const hardcore = screen.getByRole('button', { name: /Hardcore/ })
    expect(hardcore).toHaveAttribute('aria-expanded', 'false')
    fireEvent.click(hardcore)
    expect(hardcore).toHaveAttribute('aria-expanded', 'true')
    expect(cards()).toContain('d-meren')
  })

  it('previews the deck picked with its validity and last delve, and sends it only on Take', () => {
    const onTake = picker()
    const card = screen.getByRole('button', { name: /Goblin Swarm/ })
    expect(card).toHaveTextContent('Last delve 3 days ago')
    fireEvent.click(card)
    expect(card).toHaveAttribute('aria-pressed', 'true')
    const preview = screen.getByRole('complementary', { name: 'Picked deck' })
    expect(within(preview).getByText('Goblin Swarm')).toBeInTheDocument()
    expect(within(preview).getByText('Legal')).toBeInTheDocument()
    expect(onTake).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Take Goblin Swarm for Ana' }))
    expect(onTake).toHaveBeenCalledWith('d-krenko')
    expect(lastGroup()).toBe('fun')
  })

  it('marks a deck that breaks a rule and keeps Take off for the hero’s own deck', () => {
    picker('d-meren')
    expect(screen.getByRole('button', { name: /^Meren Graveyard/, pressed: true })).toHaveTextContent(
      '2 rule issues',
    )
    expect(screen.getByRole('button', { name: 'Take Meren Graveyard for Ana' })).toBeDisabled()
  })

  it('shows skeletons while the lists load', () => {
    render(
      <DeckPicker
        heroName="Ana"
        entries={null}
        current={null}
        played={{}}
        now={NOW}
        onTake={() => undefined}
        onCancel={() => undefined}
      />,
    )
    expect(screen.getByText('Loading decks')).toBeInTheDocument()
  })
})

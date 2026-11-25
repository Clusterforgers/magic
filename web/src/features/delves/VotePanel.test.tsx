// Goal: the vote for the next room is the first thing a hero sees on the map: the two camps as buttons that send the vote, the player's own vote marked, every hero's ballot (voted, still choosing, a bot following) and what the vote waits for; it hides while the party is not choosing.
import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { DelveAction, DelveView, Run } from '@/protocol/generated'
import { delveView, hero } from './fakeDelves'
import { ballots, listOf, voteWaiting } from './model'
import { VotePanel } from './VotePanel'

const ana = hero()
const bo = hero({ name: 'Bo', user: 'u1' })
const cy = hero({ name: 'Cy', user: 'u2' })
const bot = hero({ name: 'Bot', user: null, bot: 'Normal' })

function map(votes: Run['votes'], heroes = [ana, bo, bot], over: Partial<DelveView> = {}): DelveView {
  return delveView(over, { status: 'Map', at: 0, cleared: [0], votes, heroes })
}

function panel(view: DelveView) {
  const act = vi.fn<(action: DelveAction) => void>()
  render(<VotePanel view={view} act={act} busy={false} />)
  return act
}

describe('the vote panel', () => {
  it('asks for the vote with the two camps as buttons and sends the one pressed', () => {
    const act = panel(map({}))
    expect(screen.getByRole('heading', { name: 'Vote for the next room' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Vote for Event' }))
    expect(act).toHaveBeenCalledWith({ kind: 'vote', node: 2 })
    expect(screen.getByRole('button', { name: 'Vote for Shop' })).toHaveAttribute('aria-pressed', 'false')
    expect(screen.getByRole('status')).toHaveTextContent('Your vote is needed. Also waiting for Bo.')
  })

  it.each<[Run['votes'], string, string]>([
    [{ 0: 1 }, 'Waiting for Bo to vote. You can still change your vote.', 'Your vote: Shop'],
    [{ 1: 2 }, 'Everyone else voted: your vote decides.', 'Vote for Shop'],
  ])('marks the votes %o and says what it waits for', (votes, waiting, shop) => {
    panel(map(votes))
    expect(screen.getByRole('status')).toHaveTextContent(waiting)
    expect(screen.getByRole('button', { name: shop })).toHaveAttribute('aria-pressed', String(votes[0] === 1))
  })

  it('lists every hero’s ballot: voted, still choosing, or a bot that follows the players', () => {
    panel(map({ 1: 2 }, [ana, bo, cy, bot]))
    const party = within(screen.getByRole('list', { name: 'Party votes' }))
    const items = party.getAllByRole('listitem')
    expect(items.map((li) => li.getAttribute('data-ballot'))).toEqual([
      'pending',
      'voted',
      'pending',
      'follows',
    ])
    expect(items[0]).toHaveTextContent('Ana (you)')
    expect(items[1]).toHaveTextContent('Event')
    expect(items[3]).toHaveTextContent('follows the players')
    // The torch bearer after one cleared room is the second hero.
    expect(within(items[1]!).getByLabelText('torch bearer')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Vote for Event' })).toHaveTextContent('1 vote · Bo')
  })

  it.each([
    ['a fight', { status: { Fight: { game: null } } as Run['status'] }],
    ['a camp', { status: 'Room' as const }],
    ['the lobby', { status: 'Lobby' as const, at: null }],
  ])('hides during %s', (_, over) => {
    const { container } = render(
      <VotePanel view={delveView({}, { at: 0, cleared: [0], ...over })} act={() => undefined} busy={false} />,
    )
    expect(container).toBeEmptyDOMElement()
  })
})

describe('the vote helpers', () => {
  it.each([
    [[], ''],
    [['Ana'], 'Ana'],
    [['Ana', 'Bo'], 'Ana and Bo'],
    [['Ana', 'Bo', 'Cy'], 'Ana, Bo and Cy'],
  ])('lists %o as %s', (names, text) => {
    expect(listOf(names)).toBe(text)
  })

  it('says every player voted once no player is pending, while a bot still follows', () => {
    const run = map({ 0: 1, 1: 2 }).run
    expect(ballots(run).map((b) => b.state)).toEqual(['voted', 'voted', 'follows'])
    expect(voteWaiting(run, 0)).toBe('Every player voted: the party moves on.')
  })
})

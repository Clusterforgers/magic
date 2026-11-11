// Goal: the Learn page lists every tutorial with its words, a Done mark in icon and words for the finished ones (Replay instead of Start), starts a table tutorial by asking the server for a tutorial game, remembers which tutorial it plays and opens its seat link, says when the server refuses, and starts a page tour without the server.
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderInRouter } from '@/test-utils/router'
import { ApiError } from '@/net/api'
import { LearnPage } from './LearnPage'
import { endPageTour, pageTourStore } from './pageTours'
import { DONE_KEY, doneStore, TUTORIALS, tutorialOf } from './tutorials'

const SEAT = { game: 'g-tutorial', seat: 0, token: 'secret' }

function api(reply: () => Promise<typeof SEAT> = () => Promise.resolve(SEAT)) {
  return { startTutorial: vi.fn(reply) }
}

const row = (title: string): HTMLElement => screen.getByRole('listitem', { name: new RegExp(title) })

describe('LearnPage', () => {
  beforeEach(() => {
    window.localStorage.removeItem(DONE_KEY)
    window.sessionStorage.clear()
    doneStore.setState({ done: [] })
  })
  afterEach(endPageTour)

  it('lists every tutorial, with Done and Replay for the finished ones', async () => {
    doneStore.setState({ done: ['table'] })
    await renderInRouter(<LearnPage api={api()} />)
    expect(within(screen.getByRole('list', { name: 'Tutorials' })).getAllByRole('listitem')).toHaveLength(
      TUTORIALS.length,
    )
    expect(within(row('The table')).getByText('Done')).toBeInTheDocument()
    expect(within(row('The table')).getByRole('button', { name: 'Replay' })).toBeInTheDocument()
    expect(within(row('Playing a game')).queryByText('Done')).toBeNull()
    expect(within(row('Playing a game')).getByRole('button', { name: 'Start' })).toBeInTheDocument()
    expect(screen.getByText(`1 of ${TUTORIALS.length} done.`)).toBeInTheDocument()
  })

  it.each([
    ['Playing a game', 'game'],
    ['The manual panel', 'manual'],
    ['The table', 'table'],
  ] as const)('starts %s on a tutorial game and opens its seat link', async (title, id) => {
    const client = api()
    const { router } = await renderInRouter(<LearnPage api={client} />)
    await userEvent.click(within(row(title)).getByRole('button', { name: 'Start' }))
    expect(client.startTutorial).toHaveBeenCalledOnce()
    expect(tutorialOf(SEAT.game)).toBe(id)
    await vi.waitFor(() => expect(router.state.location.pathname).toBe(`/game/${SEAT.game}`))
    expect(router.state.location.hash).toBe(`seat=0&token=${SEAT.token}`)
  })

  it('says when the server refuses the tutorial game', async () => {
    const client = api(() => Promise.reject(new ApiError('server_full', 503, 'The server is full.')))
    await renderInRouter(<LearnPage api={client} />)
    await userEvent.click(within(row('Playing a game')).getByRole('button', { name: 'Start' }))
    expect(await screen.findByText('Could not start the tutorial')).toBeInTheDocument()
    expect(screen.getByText('The server is full.')).toBeInTheDocument()
  })

  it('starts a page tour without the server', async () => {
    const client = api()
    await renderInRouter(<LearnPage api={client} />)
    await userEvent.click(within(row('The rules lexicon')).getByRole('button', { name: 'Start' }))
    expect(client.startTutorial).not.toHaveBeenCalled()
    expect(pageTourStore.getState()).toMatchObject({ tour: 'lexicon', part: 0, stop: 0 })
  })
})

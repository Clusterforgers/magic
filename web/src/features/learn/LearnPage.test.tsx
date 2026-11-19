// Goal: the Learn page shows the course as a numbered path whose next lesson is the one primary button ("Start", then "Continue: <lesson>"), each lesson and app tutorial with a Done mark in icon and words, starts a lesson by asking the server for that lesson's game, remembers which lesson it plays and opens its seat link, says when the server refuses, starts the manual-panel lesson on a table, and starts a page tour without the server.
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderInRouter } from '@/test-utils/router'
import { ApiError } from '@/net/api'
import { COURSE } from './course'
import { LearnPage } from './LearnPage'
import { endPageTour, pageTourStore } from './pageTours'
import { DONE_KEY, doneStore, MANUAL_TABLE, TUTORIALS, tutorialOf } from './tutorials'

const SEAT = { game: 'g-tutorial', seat: 0, token: 'secret' }

function api(reply: () => Promise<typeof SEAT> = () => Promise.resolve(SEAT)) {
  return { startTutorial: vi.fn(reply) }
}

const lesson = (title: string): HTMLElement =>
  within(screen.getByRole('list', { name: 'Lessons' })).getByRole('listitem', { name: new RegExp(title) })
const tutorial = (title: string): HTMLElement =>
  within(screen.getByRole('list', { name: 'Tutorials' })).getByRole('listitem', { name: new RegExp(title) })

describe('LearnPage', () => {
  beforeEach(() => {
    window.localStorage.removeItem(DONE_KEY)
    window.sessionStorage.clear()
    doneStore.setState({ done: [] })
  })
  afterEach(endPageTour)

  it('lists the course in order and the app tutorials, with Start as the one primary button', async () => {
    await renderInRouter(<LearnPage api={api()} />)
    const rows = within(screen.getByRole('list', { name: 'Lessons' })).getAllByRole('listitem')
    expect(rows.map((row) => row.dataset['lesson'])).toEqual(COURSE.map((each) => each.id))
    expect(within(screen.getByRole('list', { name: 'Tutorials' })).getAllByRole('listitem')).toHaveLength(
      TUTORIALS.length,
    )
    const next = document.querySelector<HTMLElement>('[data-slot="learn-next"]')
    expect(next).toHaveTextContent('Start')
    expect(next).toHaveClass('bg-primary')
    expect(screen.getByText(`0 of ${COURSE.length} lessons done.`)).toBeInTheDocument()
  })

  it('marks finished lessons Done and continues with the next one', async () => {
    doneStore.setState({ done: ['seat', 'mana'] })
    await renderInRouter(<LearnPage api={api()} />)
    expect(within(lesson('Your seat')).getByText('Done')).toBeInTheDocument()
    expect(within(lesson('Your seat')).getByRole('button', { name: 'Replay' })).toBeInTheDocument()
    expect(within(lesson('Combat')).queryByText('Done')).toBeNull()
    expect(screen.getByRole('button', { name: 'Continue: Combat' })).toBeInTheDocument()
    expect(screen.getByText(`2 of ${COURSE.length} lessons done.`)).toBeInTheDocument()
  })

  it.each(COURSE.map((each) => [each.title, each.id] as const))(
    'starts %s on its own lesson game and opens its seat link',
    async (title, id) => {
      doneStore.setState({ done: COURSE.map((each) => each.id).filter((each) => each !== id) })
      const client = api()
      const { router } = await renderInRouter(<LearnPage api={client} />)
      expect(lesson(title)).toHaveAttribute('data-next')
      await userEvent.click(screen.getByRole('button', { name: `Continue: ${title}` }))
      expect(client.startTutorial).toHaveBeenCalledWith(id)
      expect(tutorialOf(SEAT.game)).toBe(id)
      await vi.waitFor(() => expect(router.state.location.pathname).toBe(`/game/${SEAT.game}`))
      expect(router.state.location.hash).toBe(`seat=0&token=${SEAT.token}`)
    },
  )

  it('starts the manual-panel lesson on its table', async () => {
    const client = api()
    await renderInRouter(<LearnPage api={client} />)
    await userEvent.click(within(tutorial('The manual panel')).getByRole('button', { name: 'Start' }))
    expect(client.startTutorial).toHaveBeenCalledWith(MANUAL_TABLE)
    expect(tutorialOf(SEAT.game)).toBe('manual')
  })

  it('says when the server refuses the lesson game', async () => {
    const client = api(() => Promise.reject(new ApiError('server_full', 503, 'The server is full.')))
    await renderInRouter(<LearnPage api={client} />)
    await userEvent.click(screen.getAllByRole('button', { name: 'Start' })[0] ?? document.body)
    expect(await screen.findByText('Could not start the lesson')).toBeInTheDocument()
    expect(screen.getByText('The server is full.')).toBeInTheDocument()
  })

  it('starts a page tour without the server', async () => {
    const client = api()
    await renderInRouter(<LearnPage api={client} />)
    await userEvent.click(within(tutorial('The rules lexicon')).getByRole('button', { name: 'Start' }))
    expect(client.startTutorial).not.toHaveBeenCalled()
    expect(pageTourStore.getState()).toMatchObject({ tour: 'lexicon', part: 0, stop: 0 })
  })
})

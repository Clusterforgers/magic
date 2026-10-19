// Goal: whatever goes wrong while creating a game reaches the player as one plain-words message with keyboard focus on it, leaves the form as it was, and can be retried; a request in flight cannot be sent twice.
import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { createdReply, startServer } from '@/test-utils/fakeServer'
import type { ServerScript } from '@/test-utils/fakeServer'
import { renderInRouter } from '@/test-utils/router'
import { CreateGamePage } from './CreateGamePage'

async function submitWith(script: ServerScript) {
  const server = startServer(script)
  const user = userEvent.setup()
  await renderInRouter(<CreateGamePage api={server.api} />)
  await screen.findByRole('form', { name: 'Create a game' })
  await user.clear(screen.getByLabelText('Name for seat 1'))
  await user.type(screen.getByLabelText('Name for seat 1'), 'Ana')
  await user.click(screen.getByRole('button', { name: 'Create game' }))
  return { server, user }
}

/** What the server sends for each of its error codes, and the words the page must answer with. */
const REJECTIONS: [string, number, RegExp][] = [
  ['invalid_request', 400, /could not use those game settings.*2 to 12 seats/i],
  ['unsupported_format', 400, /only play Normal games for now/i],
  ['unknown_deck', 400, /decks is no longer on the server.*reload/i],
  ['forbidden_origin', 403, /does not accept requests from this web address/i],
  ['not_found', 404, /could not find what this page asked for/i],
  ['method_not_allowed', 405, /do not agree on how to talk to each other/i],
  ['payload_too_large', 413, /too big for the server.*fewer seats or shorter names/i],
  ['unsupported_media_type', 415, /do not agree on how to talk to each other/i],
  ['internal', 500, /ran into a problem.*try again in a moment/i],
  ['server_full', 503, /as many games as it allows.*try again in a few minutes/i],
  ['unavailable', 503, /shutting down or restarting.*try again/i],
]

describe('a game the server refuses', () => {
  it.each(REJECTIONS)(
    'answers %s (HTTP %i) in plain words and moves focus to them',
    async (code, status, words) => {
      await submitWith({ create: () => jsonResponse(status, { error: code, message: 'raw server words' }) })
      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent(words)
      expect(alert).not.toHaveTextContent(code)
      expect(alert).not.toHaveTextContent('raw server words')
      expect(alert).toHaveFocus()
      expect(screen.getByLabelText('Name for seat 1')).toHaveValue('Ana')
      expect(screen.getByRole('button', { name: 'Create game' })).toBeEnabled()
      expect(screen.queryByRole('heading', { name: 'Game created' })).toBeNull()
    },
  )

  it('says the server cannot be reached when the request never gets an answer', async () => {
    await submitWith({ create: () => Promise.reject(new TypeError('Failed to fetch')) })
    expect(await screen.findByRole('alert')).toHaveTextContent(
      /could not reach the server.*check your connection/i,
    )
  })

  it('says the answer was unreadable, with its status, when a proxy answers with a page instead of JSON', async () => {
    await submitWith({ create: () => new Response('<html>Bad gateway</html>', { status: 502 }) })
    expect(await screen.findByRole('alert')).toHaveTextContent(
      /could not read \(status 502\).*down or restarting/i,
    )
  })

  it('says so when a success has the wrong shape', async () => {
    await submitWith({ create: () => jsonResponse(201, { oops: true }) })
    expect(await screen.findByRole('alert')).toHaveTextContent(/could not read \(status 201\)/i)
  })

  it('quotes a server that sends a code this page does not know', async () => {
    await submitWith({ create: () => jsonResponse(418, { error: 'teapot', message: 'short and stout' }) })
    expect(await screen.findByRole('alert')).toHaveTextContent('It said "short and stout".')
  })

  it('lets the player fix nothing and simply try again, replacing the message', async () => {
    let answers = 0
    const { server, user } = await submitWith({
      create: (body) => {
        answers += 1
        return answers === 1
          ? jsonResponse(503, { error: 'server_full', message: 'full' })
          : createdReply(body)
      },
    })
    expect(await screen.findByRole('alert')).toHaveTextContent(/as many games as it allows/i)
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    expect(await screen.findByRole('heading', { name: 'Game created' })).toBeInTheDocument()
    expect(server.creates).toHaveLength(2)
    expect(server.creates[1]).toEqual(server.creates[0])
    expect(screen.queryByRole('alert')).toBeNull()
  })
})

describe('a request in flight', () => {
  it('disables the button and says so, and sends one request however often it is asked', async () => {
    let answer: (response: Response) => void = () => undefined
    const pending = new Promise<Response>((resolve) => {
      answer = resolve
    })
    const { server, user } = await submitWith({ create: () => pending })
    const submit = screen.getByRole('button', { name: 'Creating the game…' })
    expect(submit).toHaveAttribute('aria-disabled', 'true')
    await user.click(submit)
    await user.type(screen.getByLabelText('Name for seat 2'), '{Enter}')
    expect(server.creates).toHaveLength(1)
    answer(createdReply(server.creates[0]))
    expect(await screen.findByRole('heading', { name: 'Game created' })).toBeInTheDocument()
  })

  it('sends the body that was valid when the button was pressed, not whatever is typed while it waits', async () => {
    let answer: (response: Response) => void = () => undefined
    const pending = new Promise<Response>((resolve) => {
      answer = resolve
    })
    const { server, user } = await submitWith({ create: () => pending })
    await user.type(screen.getByLabelText('Name for seat 1'), ' Banks')
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({ seats: [{ name: 'Ana' }, { name: 'Player 2' }] })
    answer(createdReply(server.creates[0]))
    await screen.findByRole('heading', { name: 'Game created' })
  })
})

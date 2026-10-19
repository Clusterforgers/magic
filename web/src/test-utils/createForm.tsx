import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { CreateGamePage } from '@/features/create/CreateGamePage'
import { startServer } from './fakeServer'
import type { ServerScript } from './fakeServer'
import { renderInRouter } from './router'

/** The create page on a fake game server, once it has loaded the decks and shows the form. */
export async function openCreateForm(script: ServerScript = {}) {
  const server = startServer(script)
  const user = userEvent.setup()
  const view = await renderInRouter(<CreateGamePage api={server.api} />)
  await screen.findByRole('form', { name: 'Create a game' })
  return { server, user, ...view }
}

/** Opens Advanced and asks for a link for every seat, the earlier way of making a game. */
export async function chooseSeatLinks(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  await user.click(screen.getByText('Advanced'))
  await user.click(screen.getByRole('checkbox', { name: 'Give every seat its own link' }))
}

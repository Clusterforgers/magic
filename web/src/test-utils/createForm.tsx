import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { CreateGamePage } from '@/features/create/CreateGamePage'
import type { GameFormat } from '@/lib/houseRules'
import { startServer } from './fakeServer'
import type { ServerScript } from './fakeServer'
import { renderInRouter } from './router'

/**
 * The create page on a fake game server, once it has loaded the decks and shows the form. The form starts on
 * Commander; `format` (Normal unless a test asks for the default) is chosen before the test begins.
 */
export async function openCreateForm(script: ServerScript = {}, format: GameFormat | 'default' = 'normal') {
  const server = startServer(script)
  const user = userEvent.setup()
  const view = await renderInRouter(<CreateGamePage api={server.api} />)
  await screen.findByRole('form', { name: 'Create a game' })
  if (format !== 'default') {
    await user.click(
      screen.getByRole('radio', { name: new RegExp(format === 'normal' ? '^Normal' : '^Commander') }),
    )
  }
  return { server, user, ...view }
}

/** Types a seat count into the Seats field, as a person does: the old text goes, then the digits. */
export async function typeSeats(
  user: ReturnType<typeof userEvent.setup>,
  count: number | string,
): Promise<void> {
  const field = screen.getByLabelText('Seats')
  await user.clear(field)
  await user.type(field, String(count))
}

/** Opens Advanced and asks for a link for every seat, the earlier way of making a game. */
export async function chooseSeatLinks(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  // A link per seat is a Normal-format flow.
  await user.click(screen.getByRole('radio', { name: /^Normal/ }))
  await user.click(screen.getByText('Advanced'))
  await user.click(screen.getByRole('checkbox', { name: 'Give every seat its own link' }))
}

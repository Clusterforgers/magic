// Goal: a pinned card preview offers "Read its rules" to a signed-in player for a card that runs on a read or leaves clauses to the table, opens ReadDialog on that card and closes itself; it offers nothing for another card or to a visitor without an account.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { ReadsApiContext } from '@/features/reads/apiContext'
import { READ_CARDS, fixtureReadsApi } from '@/features/reads/dev/fixture'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from './GameTable'

const CARD = Object.values(READ_CARDS)[0]!

function table(signedIn: boolean) {
  const auth = createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial: signedIn
      ? { status: 'signed-in', user: { id: 'ada', username: 'Ada_L', is_admin: false } }
      : { status: 'signed-out' },
  })
  return renderTable({
    ui: (
      <AuthContext value={auth}>
        <ReadsApiContext value={fixtureReadsApi()}>
          <GameTable />
        </ReadsApiContext>
      </AuthContext>
    ),
  })
}

/** Pins the opponent's first permanent after making it a card that runs on a read (when `read`). */
async function pinned(signedIn: boolean, read: boolean) {
  const harness = table(signedIn)
  const card = must(
    harness.container.querySelector<HTMLElement>('[data-battlefield-seat="1"] button[data-object-id]'),
  )
  const id = Number(card.dataset.objectId)
  harness.update((state) => {
    const object = must(state.objectsById[id])
    return {
      objectsById: {
        ...state.objectsById,
        [id]: { ...object, read, identity: { oracle_id: CARD.oracle_id } },
      },
    }
  })
  await harness.user.click(
    must(harness.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)),
  )
  return { ...harness, preview: screen.getByTestId('card-preview') }
}

describe('reading from the card preview', () => {
  it('opens ReadDialog on a card that runs on a read and closes the preview', async () => {
    const { preview, user } = await pinned(true, true)
    await user.click(within(preview).getByRole('button', { name: 'Read its rules' }))
    const dialog = await screen.findByRole('dialog', { name: /^Read the rules of / })
    expect(
      await within(dialog).findByRole('list', { name: "Clauses beside the game's reading" }),
    ).toBeVisible()
    await waitFor(() => expect(screen.queryByTestId('card-preview')).toBeNull())
  })

  it.each([
    ['a card that runs whole', true, false],
    ['a visitor without an account', false, true],
  ])('offers nothing for %s', async (_, signedIn, read) => {
    const { preview } = await pinned(signedIn, read)
    expect(within(preview).getByRole('button', { name: 'Close preview' })).toBeVisible()
    expect(within(preview).queryByRole('button', { name: 'Read its rules' })).toBeNull()
  })
})

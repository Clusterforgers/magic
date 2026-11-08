// Goal: the bulk read screen lists the wording-only cards grouped by rule, every one checked, marks the words that differ, approves exactly the checked cards in one press, says which drafts changed meanwhile, and the read page links to it only while such cards wait.
import { fireEvent, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { BulkReadsApi } from '@/net/bulkReads'
import { renderInRouter } from '@/test-utils/router'
import { BulkLink } from './BulkLink'
import { BulkReadPage } from './BulkReadPage'
import { BULK_CARDS, fixtureBulkReadsApi } from './dev/fixture'
import type { BulkFixtureOptions } from './dev/fixture'

function signedIn() {
  return createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial: { status: 'signed-in', user: { id: 'ada', username: 'Ada_L', is_admin: false } },
  })
}

async function renderPage(options: BulkFixtureOptions = {}) {
  const base = fixtureBulkReadsApi(options)
  const approve = vi.fn(base.approve)
  const api: BulkReadsApi = { ...base, approve }
  await renderInRouter(
    <AuthContext value={signedIn()}>
      <BulkReadPage api={api} />
    </AuthContext>,
  )
  await screen.findByRole('heading', { level: 1, name: 'Wording-only reads' })
  return { approve }
}

describe('BulkReadPage', () => {
  it('groups the cards by rule with every card checked', async () => {
    await renderPage()
    const groups = await screen.findAllByRole('heading', { level: 2 })
    expect(groups.map((heading) => heading.textContent)).toEqual([
      "The card's own name (2)",
      'Duration first (1)',
      'Discard your hand (1)',
    ])
    const boxes = screen.getAllByRole('checkbox')
    expect(boxes).toHaveLength(BULK_CARDS.length)
    expect(boxes.every((box) => (box as HTMLInputElement).checked)).toBe(true)
    expect(screen.getByRole('button', { name: `Approve ${BULK_CARDS.length}` })).toBeEnabled()
  })

  it('marks the words that differ on each side', async () => {
    await renderPage()
    const section = (await screen.findByRole('heading', { name: /Discard your hand/ })).closest('section')
    const marks = within(section as HTMLElement)
      .getAllByText((_, element) => element?.tagName === 'MARK')
      .map((mark) => mark.textContent?.trim())
    expect(marks).toEqual(['X cards, where X is the number of cards in'])
  })

  it('approves exactly the checked cards', async () => {
    const { approve } = await renderPage()
    fireEvent.click(await screen.findByRole('checkbox', { name: 'Darksteel Myr' }))
    fireEvent.click(screen.getByRole('button', { name: `Approve ${BULK_CARDS.length - 1}` }))
    await waitFor(() => expect(approve).toHaveBeenCalledTimes(1))
    const sent = approve.mock.calls[0]?.[0].cards.map((card) => card.oracle_id)
    expect(sent).toEqual(
      BULK_CARDS.filter((card) => card.name !== 'Darksteel Myr').map((card) => card.oracle_id),
    )
    expect(await screen.findByText(`${BULK_CARDS.length - 1} cards approved.`)).toBeInTheDocument()
    expect(await screen.findByRole('checkbox', { name: 'Darksteel Myr' })).toBeChecked()
    expect(screen.getAllByRole('checkbox')).toHaveLength(1)
  })

  it('says when drafts changed since the page loaded', async () => {
    await renderPage({ stale: true })
    fireEvent.click(await screen.findByRole('button', { name: `Approve ${BULK_CARDS.length}` }))
    expect(
      await screen.findByText(
        `0 cards approved. ${BULK_CARDS.length} cards changed since the page loaded and left alone.`,
      ),
    ).toBeInTheDocument()
  })

  it('shows an empty state when nothing waits', async () => {
    await renderPage({ empty: true })
    expect(await screen.findByText('Nothing to approve in bulk')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Approve/ })).toBeNull()
  })
})

describe('BulkLink', () => {
  it('links to the bulk screen with the count, and hides while nothing waits', async () => {
    const { unmount } = await renderInRouter(<BulkLink api={fixtureBulkReadsApi()} />)
    expect(
      await screen.findByRole('link', { name: `${BULK_CARDS.length} wording-only, approve in bulk` }),
    ).toHaveAttribute('href', '/read/bulk')
    unmount()
    await renderInRouter(<BulkLink api={fixtureBulkReadsApi({ empty: true })} />)
    await waitFor(() => expect(screen.queryByRole('link')).toBeNull())
  })
})

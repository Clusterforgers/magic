import { useSearch } from '@tanstack/react-router'
import { useMemo, useState } from 'react'
import { PageShell } from '@/components/PageShell'
import { Button } from '@/components/ui/button'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'

import { devCheckCards, devCheckDecks } from './checkFixture'
import { cardLine, preconItem, preconView, reason } from '@/features/precons/fakePrecons'
import { CardsApiContext } from '@/features/table/cardsContext'
import { cardsApi } from '@/net/cards'

import type { PreconsApi } from '@/net/precons'
import { ReadsApiContext } from '../apiContext'
import { ReadDialog } from '../ReadDialog'
import { ReadPage } from '../ReadPage'
import { READ_CARDS, READS_FIXTURE_MARKER, fixtureReadBulkApi, fixtureReadsApi } from './fixture'
import type { ReadFixtureOptions } from './fixture'
import type { DevReadView } from './readSearch'

function optionsOf(view: DevReadView): ReadFixtureOptions {
  switch (view) {
    case 'done':
      return { empty: true }
    case 'loading':
      return { loading: true }
    case 'error':
      return { listFails: 'network' }
    case 'stale':
      return { stale: true }
    case 'long':
      return { long: true }
    default:
      return { delayMs: 150 }
  }
}

const FIRST = Object.values(READ_CARDS)[0]

/** The table's way in: a button that opens `ReadDialog` on the first fixture card, open from the start. */
function DialogScene() {
  const [open, setOpen] = useState(true)
  return (
    <PageShell width="wide">
      <Button onClick={() => setOpen(true)}>Read {FIRST?.name}</Button>
      <ReadDialog
        oracleId={open ? (FIRST?.oracle_id ?? null) : null}
        name={FIRST?.name ?? ''}
        onClose={() => setOpen(false)}
      />
    </PageShell>
  )
}

/**
 * `/dev/read?view=deck`: the real review screen over made-up cards in memory, signed in. `view` is `deck`, `precon`, `all`,
 * `done` (nothing left to read), `loading`, `error`, `stale` (every verdict meets a changed draft), `dialog` (the
 * table's `ReadDialog`) or `long` (55 cards to read, one with a name far wider than the list).
 */
export function DevReadRoute() {
  const { view } = useSearch({ from: '/dev/read' })
  const api = useMemo(() => fixtureReadsApi(optionsOf(view)), [view])
  const world = useMemo(() => {
    // Nothing to check either in the done view; the deck of the others holds two cards played by hand.
    const decks = devCheckDecks(view === 'done')
    const precons: Pick<PreconsApi, 'get'> = {
      get: async (code) =>
        preconView(preconItem({ code, name: 'Adaptive Enchantment' }), [
          cardLine('Mana Drain', 'partial', {
            reasons: [reason('The delayed mana is not implemented', 30, 120)],
          }),
          cardLine('Rhystic Study', 'manual'),
          cardLine('Sol Ring', 'verified'),
        ]),
    }
    return {
      checks: { decks, precons },
      byHand: decks,
      cards: devCheckCards(cardsApi),
      bulk: fixtureReadBulkApi(),
    }
  }, [view])
  const auth = useMemo(
    () =>
      createAuthStore({
        api: { ...offlineAccounts, myGames: async () => [] },
        initial: { status: 'signed-in', user: { id: 'dev-ada', username: 'Ada_L', is_admin: false } },
      }),
    [],
  )
  return (
    <AuthContext value={auth}>
      <CardsApiContext value={world.cards}>
        <ReadsApiContext value={api}>
          <p className="sr-only" data-fixture={READS_FIXTURE_MARKER}>
            Development review screen
          </p>
          {view === 'dialog' ? (
            <DialogScene />
          ) : (
            <ReadPage
              key={view}
              api={api}
              checks={world.checks}
              decks={world.byHand}
              bulk={world.bulk}
              target={
                view === 'mine'
                  ? { mine: true }
                  : view === 'all'
                    ? null
                    : view === 'precon'
                      ? { precon: 'c18-adaptive-enchantment' }
                      : { deck: 'deck-izzet' }
              }
            />
          )}
        </ReadsApiContext>
      </CardsApiContext>
    </AuthContext>
  )
}

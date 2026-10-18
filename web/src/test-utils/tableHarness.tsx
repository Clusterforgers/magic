import { act, render } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactElement } from 'react'
import { GameContext } from '@/features/game/gameContext'
import type { GameContextValue } from '@/features/game/gameContext'
import type { GameLookup } from '@/features/game/useGameInfo'
import { fixtureLookup, fixtureSession, fixtureState } from '@/features/table/dev/devGame'
import { buildFixture } from '@/features/table/dev/fixture'
import type { Fixture } from '@/features/table/dev/fixture'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import type { TableOptions } from '@/features/table/tableOptions'
import type { Session } from '@/play/session'
import { createPlayStore } from '@/play/store'
import type { PlayState, PlayStore } from '@/play/store'

/** What a table test sets up. */
export interface TableSetup {
  /** What to show; `buildFixture({ seats })` by default. */
  fixture?: Fixture
  /** Seats of the default fixture; 2 by default. */
  seats?: number
  /** Where the layout starts. */
  options?: TableOptions
  /** What REST says about the game; every seat named and connected by default. */
  lookup?: GameLookup
  /** Parts of the session to replace, such as `chat` with a spy. */
  session?: Partial<Session>
  /** Changes the play state before the first render. */
  state?: (state: PlayState) => PlayState
  /** What to render instead of `GameTable` inside the game; the same providers wrap it. */
  ui?: ReactElement
}

/** A table on a fixture game with no server, and the means to change what it shows. */
export interface TableHarness {
  store: PlayStore
  session: Session
  lookup: GameLookup
  fixture: Fixture
  user: ReturnType<typeof userEvent.setup>
  container: HTMLElement
  /** Applies a change to the play state the way a batch would. */
  update(change: (state: PlayState) => Partial<PlayState>): void
  unmount(): void
}

/** Renders `GameTable` (or `setup.ui`) inside a game context built from a fixture. */
export function renderTable(setup: TableSetup = {}): TableHarness {
  const fixture = setup.fixture ?? buildFixture({ seats: setup.seats ?? 2 })
  const store = createPlayStore()
  const first = fixtureState(fixture)
  store.setState(setup.state === undefined ? first : setup.state(first))
  const session: Session = { ...fixtureSession(store), ...setup.session }
  const lookup = setup.lookup ?? fixtureLookup(fixture)
  const value: GameContextValue = { game: 'test-game', session, lookup }
  const view = render(
    <GameContext value={value}>
      <TableOptionsContext value={setup.options ?? {}}>{setup.ui ?? <GameTable />}</TableOptionsContext>
    </GameContext>,
  )
  return {
    store,
    session,
    lookup,
    fixture,
    user: userEvent.setup(),
    container: view.container,
    update: (change) => act(() => store.setState(change)),
    unmount: view.unmount,
  }
}

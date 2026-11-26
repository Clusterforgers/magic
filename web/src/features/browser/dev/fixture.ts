/** Made-up open games for `/dev/play`: every kind of row the list draws, and a long list for the filters. */
import type { LobbyGame, MyGame } from '@/protocol/generated'

/** Text only the dev Play page holds; `devBuild.test.ts` checks that no production file contains it. */
export const PLAY_FIXTURE_MARKER = 'play-fixture'

/** A host name as long as a username may be, for the cut-off rules. */
export const LONG_HOST = 'Bartholomew_Featherstonehaugh_3'

const game = (over: Partial<LobbyGame> & Pick<LobbyGame, 'game' | 'host'>): LobbyGame => ({
  format: 'normal',
  manual_play: false,
  filled: 1,
  total: 2,
  created_at: '2026-10-19T12:00:00Z',
  age_secs: 120,
  ...over,
})

/** One row of every kind: open, Commander with commanders, played by hand, house rules, full, running, seated, a long host. */
export const FIXTURE_LOBBIES: readonly LobbyGame[] = [
  game({ game: 'dev-open', host: 'Bo', age_secs: 20 }),
  game({
    game: 'dev-commander',
    host: 'Cy',
    format: 'commander',
    filled: 2,
    total: 4,
    age_secs: 200,
    commanders: [
      { seat: 0, name: "Atraxa, Praetors' Voice" },
      { seat: 1, name: 'Tymna the Weaver' },
    ],
  }),
  game({ game: 'dev-manual', host: 'Dee', manual_play: true, filled: 1, total: 3, age_secs: 900 }),
  game({
    game: 'dev-rules',
    host: 'Eli',
    format: 'commander',
    house_rules: { starting_life: 30, free_mulligans: 1 },
    filled: 3,
    total: 6,
    age_secs: 2_400,
  }),
  game({ game: 'dev-full', host: 'Fay', filled: 2, total: 2, age_secs: 3_700 }),
  game({
    game: 'dev-running',
    host: 'Gus',
    format: 'commander',
    filled: 4,
    total: 4,
    running: true,
    age_secs: 5_400,
  }),
  game({ game: 'dev-seated', host: 'Hana', filled: 2, total: 4, seated: true, age_secs: 7_200 }),
  game({
    game: 'dev-long',
    host: LONG_HOST,
    format: 'two_headed_giant',
    filled: 1,
    total: 4,
    age_secs: 90_000,
  }),
]

/** `count` open games beyond the fixture rows, for a list long enough to scroll and filter. */
export const manyLobbies = (count: number): LobbyGame[] => [
  ...FIXTURE_LOBBIES,
  ...Array.from({ length: count }, (_, index) =>
    game({
      game: `dev-more-${index + 1}`,
      host: `Player ${index + 1}`,
      format: index % 3 === 0 ? 'commander' : 'normal',
      filled: 1 + (index % 3),
      total: 4,
      age_secs: 600 * (index + 1),
    }),
  ),
]

/** The signed-in player's games: one running and one waiting in its lobby, which the page offers to continue. */
export const FIXTURE_MY_GAMES: readonly MyGame[] = [
  {
    game: 'dev-mine-running',
    seat: 0,
    name: 'Ada_L',
    status: 'running',
    created_at: '2026-10-19T11:00:00Z',
    others: [{ seat: 1, name: 'Bo', user: 'Bo' }],
    result: null,
    reason: null,
  },
  {
    game: 'dev-mine-lobby',
    seat: 0,
    name: 'Ada_L',
    status: 'lobby',
    created_at: '2026-10-19T11:50:00Z',
    others: [],
    result: null,
    reason: null,
  },
]

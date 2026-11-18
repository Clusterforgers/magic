/** The Delves leaderboard: each player's deepest run, one tab per Heat from Heroic down. */
import { TrophyIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Loading } from '@/components/Loading'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { DelvesApi } from '@/net/delves'
import type { DelveBoard, DelveBoardEntry } from '@/protocol/generated'
import { depthWords, heatTier, orderBoards } from './model'
import { errorText } from './words'

function Entry({ entry, rank }: { entry: DelveBoardEntry; rank: number }) {
  const party = entry.party === 1 ? 'Solo' : `Party of ${entry.party}`
  return (
    <li className="grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-baseline gap-x-2 border-b py-2 last:border-b-0">
      <span className="text-sm text-muted-foreground tabular-nums">{rank}</span>
      <span className="truncate text-sm font-medium">{entry.player}</span>
      <span className="text-sm font-medium tabular-nums">{depthWords(entry)}</span>
      <span className="col-span-2 col-start-2 text-xs">{entry.commanders.join(' & ')}</span>
      <span className="col-span-2 col-start-2 text-xs text-muted-foreground tabular-nums">
        {party} · <time dateTime={entry.date}>{entry.date}</time>
      </span>
    </li>
  )
}

/** The leaderboard panel; it reads the boards once on mount. */
export function DelvesLeaderboard({ client }: { client: DelvesApi }) {
  const [boards, setBoards] = useState<DelveBoard[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    client
      .leaderboard()
      .then((answer) => live && setBoards(orderBoards(answer.boards)))
      .catch((caught: unknown) => live && setError(errorText(caught)))
    return () => {
      live = false
    }
  }, [client])
  return (
    <aside aria-labelledby="delves-leaderboard" className="surface grid content-start gap-3 p-3">
      <div className="grid gap-1">
        <h2 id="delves-leaderboard" className="flex items-center gap-2 text-base font-semibold">
          <TrophyIcon aria-hidden="true" className="size-4 text-relic" />
          Leaderboard
        </h2>
        <p className="text-xs text-muted-foreground">
          Each player's deepest run: floors cleared, then bosses beaten below them.
        </p>
      </div>
      {error !== null ? (
        <p role="alert" className="text-sm text-muted-foreground">
          Could not load the leaderboard. {error}
        </p>
      ) : boards === null ? (
        <Loading label="Loading the leaderboard…">
          <Skeleton className="h-24" />
        </Loading>
      ) : boards.length === 0 ? null : (
        <Tabs defaultValue={String(boards[0]?.heat ?? 2)}>
          <TabsList aria-label="Difficulty" className="grid w-full grid-cols-3">
            {boards.map((board) => (
              <TabsTrigger key={board.heat} value={String(board.heat)}>
                {heatTier(board.heat).name}
              </TabsTrigger>
            ))}
          </TabsList>
          {boards.map((board) => (
            <TabsContent key={board.heat} value={String(board.heat)} className="outline-none">
              {board.entries.length === 0 ? (
                <p className="py-2 text-sm text-muted-foreground">
                  No run at {heatTier(board.heat).name} yet.
                </p>
              ) : (
                <ol aria-label={`${heatTier(board.heat).name} leaderboard`}>
                  {board.entries.map((entry, at) => (
                    <Entry key={`${entry.player}-${at}`} entry={entry} rank={at + 1} />
                  ))}
                </ol>
              )}
            </TabsContent>
          ))}
        </Tabs>
      )}
    </aside>
  )
}

/**
 * The waiting room of a Delves boss fight, in place of the game's lobby: the boss's intro and one button, Ready to
 * fight, with who is at the table. There is no deck choice: each hero plays the deck the run chose.
 */
import { Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useGame, usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { delvesApi } from '@/net/delves'
import type { DelvesApi } from '@/net/delves'
import type { DelveContent, DelveView, LobbySeat } from '@/protocol/generated'
import { BossIntro } from './BossIntro'

/** One empty roster, so the selector's answer stays the same while there is none. */
const NO_SEATS: readonly LobbySeat[] = []

/** The fight's run and the campaign content, read once. */
function useRun(client: DelvesApi, delve: string): { view: DelveView | null; content: DelveContent | null } {
  const [view, setView] = useState<DelveView | null>(null)
  const [content, setContent] = useState<DelveContent | null>(null)
  useEffect(() => {
    let live = true
    client
      .get(delve)
      .then((answer) => live && setView(answer))
      .catch(() => undefined)
    client
      .content()
      .then((answer) => live && setContent(answer))
      .catch(() => undefined)
    return () => {
      live = false
    }
  }, [client, delve])
  return { view, content }
}

/** The waiting room of the boss fight of run `delve`. */
export function FightLobby({ delve, client = delvesApi }: { delve: string; client?: DelvesApi }) {
  const { session } = useGame()
  const { view, content } = useRun(client, delve)
  const { seat, seats } = usePlay(
    useShallow((state) => ({ seat: state.seat, seats: state.lobby?.seats ?? NO_SEATS })),
  )
  const mine = seats.find((entry) => entry.seat === seat)
  const ready = mine?.ready === true
  const heroes = view?.run.heroes.length ?? 0
  const party = seats.filter((entry) => entry.seat < heroes)
  const people = view?.run.heroes.filter((hero) => hero.bot === null).length ?? 0
  const waiting = party.filter((entry) => !entry.ready && view?.run.heroes[entry.seat]?.bot === null)
  const back = (
    <Link to="/delves/$delveId" params={{ delveId: delve }} className={buttonVariants({ variant: 'ghost' })}>
      Back to the run
    </Link>
  )
  if (view === null || content === null) {
    return (
      <PageShell width="page">
        <Loading label="Reading the boss fight…">
          <Skeleton className="h-48" />
        </Loading>
      </PageShell>
    )
  }
  return (
    <PageShell width="wide" className="grid gap-4 sm:py-6">
      <BossIntro view={view} content={content}>
        <div data-slot="fight-ready" className="grid gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <Button
              size="lg"
              variant={ready ? 'outline' : 'default'}
              className={cn('w-full sm:w-auto')}
              onClick={() => (ready ? session.notReady() : session.ready())}
            >
              {ready ? 'Not ready yet' : 'Ready to fight'}
            </Button>
            {back}
          </div>
          <p role="status" className="text-sm text-muted-foreground">
            {waiting.length === 0
              ? 'Every hero is ready: the fight begins.'
              : `${people - waiting.length} of ${people} players ready · waiting for ${waiting
                  .map((entry) => entry.name)
                  .join(', ')}. Bot allies are always ready.`}
          </p>
        </div>
      </BossIntro>
    </PageShell>
  )
}

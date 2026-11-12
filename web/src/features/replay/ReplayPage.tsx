import { Link } from '@tanstack/react-router'
import { ChevronFirst, ChevronLast, ChevronLeft, ChevronRight, Pause, Play } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Button, buttonVariants } from '@/components/ui/button'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { GameContext } from '@/features/game/gameContext'
import type { GameContextValue } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import type { TableOptions } from '@/features/table/tableOptions'
import { usePageTitle } from '@/hooks/usePageTitle'
import { cn } from '@/lib/utils'
import type { Fetch } from '@/net/api'
import { loadReplay, ReplayError } from '@/net/replays'
import { createPlayStore, initialPlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { PlayerId, Replay } from '@/protocol/generated'
import { createCursor, startSeat, withPerspective } from './replayModel'
import { replaySession } from './replaySession'

/** How long each step shows while the replay plays. */
export const PLAY_STEP_MS = 900

/** The table options of a replay: no sound cues and no tour; the animations of each step play as at the table. */
const OPTIONS: TableOptions = { sounds: false, tour: false }

type Load =
  | { state: 'loading' }
  | { state: 'failed'; reason: ReplayError['reason'] }
  | { state: 'ready'; replay: Replay }

/** What a replay page is given; `fetch` and `options` stand in for the network and the window in tests. */
interface ReplayPageProps {
  game: string
  fetch?: Fetch
  options?: TableOptions
}

/** `/replay/$gameId`: a finished game step by step, from any seat's side, with what the viewer may know. */
export function ReplayPage({ game, fetch, options }: ReplayPageProps) {
  usePageTitle('Replay')
  const [load, setLoad] = useState<Load>({ state: 'loading' })
  useEffect(() => {
    const abort = new AbortController()
    loadReplay(game, fetch, abort.signal).then(
      (replay) => setLoad({ state: 'ready', replay }),
      (error: unknown) => {
        if (abort.signal.aborted) return
        setLoad({ state: 'failed', reason: error instanceof ReplayError ? error.reason : 'failed' })
      },
    )
    return () => abort.abort()
  }, [game, fetch])

  if (load.state === 'loading') {
    return (
      <PageShell width="narrow" className="sm:pt-12">
        <Loading label="Loading the replay…">{null}</Loading>
      </PageShell>
    )
  }
  if (load.state === 'failed') {
    return (
      <PageShell width="narrow" className="sm:pt-12">
        <EmptyState
          as="h1"
          title={
            load.reason === 'not_found'
              ? 'No replay for this game'
              : load.reason === 'signed_out'
                ? 'Sign in to watch replays'
                : 'The replay could not be loaded'
          }
          action={
            <Link to="/games" className={cn(buttonVariants({ size: 'lg' }))}>
              My games
            </Link>
          }
        >
          {load.reason === 'not_found'
            ? 'A game has a replay once it has ended.'
            : load.reason === 'signed_out'
              ? 'Replays are for signed-in players, like watching a running game.'
              : load.reason === 'network'
                ? 'Could not reach the server. Check your connection and try again.'
                : 'Something went wrong on the server. Try again later.'}
        </EmptyState>
      </PageShell>
    )
  }
  return <ReplayTable game={game} replay={load.replay} options={options} />
}

/** The table of a loaded replay with its controls. */
function ReplayTable({ game, replay, options }: { game: string; replay: Replay; options?: TableOptions }) {
  const tableOptions = useMemo(() => ({ ...OPTIONS, ...options }), [options])
  const cursor = useMemo(() => createCursor(replay.steps), [replay])
  const last = replay.steps.length - 1
  const [index, setIndex] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [seat, setSeat] = useState<PlayerId>(() => startSeat(replay))
  const [store] = useState(createPlayStore)
  const context = useMemo<GameContextValue>(() => {
    const seats = replay.names.map((name, at) => ({ seat: at, name }))
    return {
      game,
      session: replaySession(store),
      lookup: {
        game: { started: true, seats, connected: [], ready: [], result: replay.result ?? null },
        missing: false,
        failed: false,
      },
    }
  }, [game, replay, store])

  useEffect(() => {
    if (cursor === null) return
    const frame = cursor(index)
    store.setState({
      ...initialPlayState(),
      status: 'open',
      seat,
      seatCount: replay.names.length,
      view: withPerspective(frame.view, seat),
      objectsById: indexById(frame.view.objects),
      seq: frame.seq,
      log: frame.log,
      phase: 'playing',
    })
  }, [cursor, index, seat, replay, store])

  // Playing stops by itself at the last step.
  const running = playing && index < last
  useEffect(() => {
    if (!running) return undefined
    const timer = window.setTimeout(() => setIndex((at) => Math.min(last, at + 1)), PLAY_STEP_MS)
    return () => window.clearTimeout(timer)
  }, [running, index, last])

  if (cursor === null) {
    return (
      <PageShell width="narrow" className="sm:pt-12">
        <EmptyState as="h1" title="The replay could not be loaded">
          The server sent a replay this page cannot read.
        </EmptyState>
      </PageShell>
    )
  }
  const go = (to: number) => {
    setPlaying(false)
    setIndex(Math.max(0, Math.min(last, to)))
  }
  return (
    <GameContext value={context}>
      <TableOptionsContext value={tableOptions}>
        <GameTable />
        <div
          role="toolbar"
          aria-label="Replay"
          data-slot="replay-controls"
          className="fixed right-2 bottom-2 left-2 z-50 flex flex-wrap items-center justify-center gap-1 rounded-lg border bg-background/95 px-2 py-1 shadow-md sm:left-auto sm:max-w-[34rem]"
        >
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="First step"
            disabled={index === 0}
            onClick={() => go(0)}
          >
            <ChevronFirst aria-hidden="true" />
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="Step back"
            disabled={index === 0}
            onClick={() => go(index - 1)}
          >
            <ChevronLeft aria-hidden="true" />
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label={running ? 'Pause' : 'Play'}
            data-slot="replay-play"
            disabled={index === last}
            onClick={() => setPlaying(!running)}
          >
            {running ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="Step forward"
            disabled={index === last}
            onClick={() => go(index + 1)}
          >
            <ChevronRight aria-hidden="true" />
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="Last step"
            disabled={index === last}
            onClick={() => go(last)}
          >
            <ChevronLast aria-hidden="true" />
          </Button>
          <input
            type="range"
            aria-label="Step"
            className="w-32 sm:w-48"
            min={0}
            max={last}
            value={index}
            onChange={(event) => go(Number(event.target.value))}
          />
          <span data-slot="replay-step" className="px-1 text-xs tabular-nums text-muted-foreground">
            {index + 1} / {last + 1}
          </span>
          <NativeSelect
            size="sm"
            aria-label="Seen from"
            data-slot="replay-seat"
            value={String(seat)}
            onChange={(event) => setSeat(Number(event.target.value))}
          >
            {replay.names.map((name, at) => (
              <NativeSelectOption key={at} value={String(at)}>
                {`Seen from ${name}${replay.seat === at ? ' (you)' : ''}`}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
      </TableOptionsContext>
    </GameContext>
  )
}

/**
 * `/delves/$delveId`: one run, in the dungeon's look. The run header (Heat, hearts, seed, the party's life and relics),
 * then the stage of the run: the lobby with the deck picker, the boss fight (its portrait, card, phases and a link to
 * the table), the camp scenes, or the end; then the descent with the votes on the next camp. The page reads the run
 * every few seconds, which also reads a finished fight back.
 */
import { Link } from '@tanstack/react-router'
import { CircleAlertIcon, ShieldIcon, SwordsIcon, TrophyIcon } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { usePageTitle } from '@/hooks/usePageTitle'
import { delvesApi } from '@/net/delves'
import type { DelvesApi } from '@/net/delves'
import { cn } from '@/lib/utils'
import type { DelveAction, DelveBossView, DelveContent, DelveView } from '@/protocol/generated'
import { BossLadder } from './BossLadder'
import { BossPortrait, ColourPips } from './CommanderArt'
import { CustomCardFrame } from './CustomCardFrame'
import { DelveOffer } from './DelveOffer'
import { loadPickerDecks, notePlayed, readPlayed } from './deckPicker'
import type { LastPlayed, PickerDecks } from './deckPicker'
import { bossLife } from './model'
import type { FindCard } from './findCard'
import { PartyLobby } from './PartyLobby'
import { RunHeader } from './RunHeader'
import { POLL_MS, errorText } from './words'

function bossOf(content: DelveContent, id: string | null | undefined): DelveBossView | undefined {
  return content.bosses.find((b) => b.id === id)
}

/** The boss's card and its phases, with the share of its life at which each begins. */
export function BossPanel({ content, boss }: { content: DelveContent; boss: DelveBossView }) {
  const card = content.cards.find((c) => c.id === boss.card)
  return (
    <section aria-label={`Boss: ${boss.name}`} className="grid gap-2">
      <ol className="flex flex-wrap justify-center gap-3 sm:justify-start">
        {card ? (
          <li className="grid gap-1">
            <span className="text-xs font-medium text-muted-foreground">From the start</span>
            <CustomCardFrame card={card} />
          </li>
        ) : null}
        {boss.phases.map(([id, percent], i) => {
          const phase = content.cards.find((c) => c.id === id)
          return phase ? (
            <li key={id} className="grid gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                Phase {i + 2} · At {percent}% of its life
              </span>
              <CustomCardFrame card={phase} />
            </li>
          ) : null
        })}
      </ol>
    </section>
  )
}

function FightPanel({ view, content }: { view: DelveView; content: DelveContent }) {
  const status = view.run.status
  const game = typeof status === 'object' && 'Fight' in status ? status.Fight.game : null
  const boss = bossOf(content, view.boss)
  return (
    <section
      aria-label="Boss fight"
      className="grid gap-4 rounded-xl border-2 border-delve bg-delve-stone p-4 shadow-e2 motion-safe:animate-in motion-safe:fade-in motion-safe:duration-slow sm:p-6"
    >
      <div className="flex flex-wrap items-center gap-4">
        <BossPortrait
          colours={boss?.colors ?? ''}
          className="size-20 shrink-0 rounded-full border-2 border-delve"
        />
        <div className="grid min-w-0 flex-1 gap-1">
          <p className="inline-flex items-center gap-1 text-xs font-semibold tracking-wide text-delve uppercase">
            <SwordsIcon aria-hidden="true" className="size-4" />
            Boss fight
          </p>
          <h2 className="text-2xl font-semibold">{boss ? boss.name : 'A boss'} awaits</h2>
          {boss ? (
            <p className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
              <ColourPips colours={boss.colors} />
              <span>{bossLife(boss.life, view.run.heroes.length, view.run.heat)} life</span>
              <span>· {boss.phases.length + 1} phases</span>
            </p>
          ) : null}
          {boss ? <p className="text-sm italic">“{boss.line}”</p> : null}
        </div>
        {game ? (
          <Link
            to="/game/$gameId"
            params={{ gameId: game }}
            className={cn(buttonVariants({ size: 'lg' }), 'w-full sm:w-auto')}
          >
            Go to the fight
          </Link>
        ) : (
          <p role="status" className="text-sm text-muted-foreground">
            The fight is being prepared…
          </p>
        )}
      </div>
      {boss ? <BossPanel content={content} boss={boss} /> : null}
    </section>
  )
}

function EndPanel({ view }: { view: DelveView }) {
  const status = view.run.status
  const won = status === 'Victory'
  const Icon = won ? TrophyIcon : ShieldIcon
  return (
    <section
      aria-label="The end"
      className={cn(
        'grid justify-items-center gap-3 rounded-xl border-2 bg-delve-stone p-6 text-center shadow-e2',
        won ? 'border-relic' : 'border-border-strong',
      )}
    >
      <Icon aria-hidden="true" className={cn('size-12', won ? 'text-relic' : 'text-muted-foreground')} />
      <h2 className="text-2xl font-semibold">
        {won ? 'Victory!' : status === 'Defeat' ? 'The party fell' : 'The party turned back'}
      </h2>
      <p className="max-w-prose text-sm text-muted-foreground">
        Seed code <strong className="font-mono text-foreground">{view.seed}</strong> replays this ladder for
        another party.
      </p>
      <CopyButton text={view.seed} label="Copy seed code">
        Copy seed code
      </CopyButton>
    </section>
  )
}

function RoomPanel({
  view,
  content,
  act,
  busy,
  find,
}: {
  view: DelveView
  content: DelveContent
  act: (action: DelveAction) => void
  busy: boolean
  find?: FindCard
}) {
  const me = view.me === undefined ? undefined : view.run.heroes[view.me]
  const offer = me?.offers[0]
  const waiting = view.run.heroes.filter((h) => h.offers.length > 0).map((h) => h.name)
  return (
    <div className="grid gap-3">
      {offer && me ? (
        <DelveOffer
          offer={offer}
          gold={me.gold}
          life={me.life}
          content={content}
          busy={busy}
          find={find}
          onPick={(pick) => act({ kind: 'pick', pick })}
        />
      ) : null}
      {waiting.length > 0 && !offer ? (
        <p role="status" className="rounded-xl border bg-surface p-4 text-sm text-muted-foreground">
          Waiting for {waiting.join(', ')}.
        </p>
      ) : null}
    </div>
  )
}

/** Reads the run and the content, keeps the run fresh and sends actions. */
function useDelve(client: DelvesApi, id: string) {
  const [view, setView] = useState<DelveView | null>(null)
  const [content, setContent] = useState<DelveContent | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    let live = true
    const read = () =>
      client
        .get(id)
        .then((answer) => live && setView(answer))
        .catch((caught: unknown) => live && setError(errorText(caught)))
    client
      .content()
      .then((answer) => live && setContent(answer))
      .catch((caught: unknown) => live && setError(errorText(caught)))
    void read()
    const timer = setInterval(() => void read(), POLL_MS)
    return () => {
      live = false
      clearInterval(timer)
    }
  }, [client, id])
  const act = useCallback(
    (action: DelveAction) => {
      setBusy(true)
      setError(null)
      client
        .act(id, action)
        .then(setView)
        .catch((caught: unknown) => setError(errorText(caught)))
        .finally(() => setBusy(false))
    },
    [client, id],
  )
  return { view, content, error, busy, act }
}

/** The run page; `/delves/$delveId` renders it with the id of its path. */
export function DelvePage({
  id,
  client = delvesApi,
  loadDecks = loadPickerDecks,
  find,
}: {
  id: string
  client?: DelvesApi
  loadDecks?: () => Promise<PickerDecks>
  /** Finds offered cards' pictures; the card search by default. */
  find?: FindCard
}) {
  usePageTitle('Delve')
  const { view, content, error, busy, act } = useDelve(client, id)
  const [decks, setDecks] = useState<PickerDecks | null>(null)
  const [played, setPlayed] = useState<LastPlayed>(() => readPlayed())
  useEffect(() => {
    let live = true
    loadDecks()
      .then((list) => live && setDecks(list))
      .catch(() => live && setDecks({ entries: [], groups: [] }))
    return () => {
      live = false
    }
  }, [loadDecks])
  if (view === null || content === null) {
    if (error !== null) {
      return (
        <PageShell width="page">
          <Alert variant="destructive">
            <CircleAlertIcon aria-hidden="true" />
            <AlertTitle>Could not load the delve</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        </PageShell>
      )
    }
    return (
      <PageShell width="wide">
        <Loading label="Loading the delve…">
          <Skeleton className="h-32" />
          <Skeleton className="mt-4 h-64" />
        </Loading>
      </PageShell>
    )
  }
  const status = view.run.status
  const fighting = typeof status === 'object' && 'Fight' in status
  const over = status === 'Victory' || status === 'Defeat' || status === 'Abandoned'
  const ladder = <BossLadder view={view} content={content} act={act} busy={busy} />
  return (
    <PageShell width="wide" className="grid gap-4 sm:py-6">
      <RunHeader view={view} content={content} />
      {error !== null ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="grid min-w-0 content-start gap-4">
          {status === 'Lobby' ? (
            <>
              <PartyLobby
                view={view}
                act={act}
                busy={busy}
                entries={decks?.entries ?? null}
                groups={decks?.groups ?? []}
                played={played}
                onPlayed={(deck) => setPlayed(notePlayed(deck, new Date()))}
              />
              <section aria-label={content.theme} className="rounded-xl border bg-delve-stone p-4 text-sm">
                {content.text}
              </section>
            </>
          ) : null}
          {fighting ? <FightPanel view={view} content={content} /> : null}
          {status === 'Room' ? (
            <RoomPanel view={view} content={content} act={act} busy={busy} find={find} />
          ) : null}
          {over ? <EndPanel view={view} /> : null}
          {status !== 'Lobby' ? ladder : null}
        </div>
        <aside className="grid content-start gap-4">
          <section aria-labelledby="log-title" className="surface grid gap-2 p-3 text-sm">
            <h2 id="log-title" className="font-semibold">
              Chronicle
            </h2>
            <ol className="grid gap-1 text-muted-foreground">
              {view.run.log.slice(-8).map((line, i) => (
                <li key={`${i}-${line}`} className="border-l-2 border-border pl-2">
                  {line}
                </li>
              ))}
            </ol>
          </section>
          {view.host && !over && status !== 'Lobby' ? (
            <Button variant="ghost" disabled={busy} onClick={() => act({ kind: 'abandon' })}>
              End the run
            </Button>
          ) : null}
        </aside>
      </div>
    </PageShell>
  )
}

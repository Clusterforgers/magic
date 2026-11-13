/**
 * `/delves/$delveId`: one run. The lobby (heroes, decks, bot allies), the boss ladder with the party's votes on the
 * next camp, the boss fight (its card, phases and a link to the table), the loot, shop, campfire and event screens,
 * and the end. The page reads the run every few seconds, which also reads a finished fight back.
 */
import { Link } from '@tanstack/react-router'
import {
  CircleAlertIcon,
  FlameIcon,
  HeartIcon,
  ShieldIcon,
  SkullIcon,
  SwordsIcon,
  TrophyIcon,
} from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Pill } from '@/components/Pill'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Skeleton } from '@/components/ui/skeleton'
import { usePageTitle } from '@/hooks/usePageTitle'
import { delvesApi } from '@/net/delves'
import type { DelvesApi } from '@/net/delves'
import { cn } from '@/lib/utils'
import type {
  Brain,
  DelveAction,
  DelveBossView,
  DelveContent,
  DelveView,
  FloorMap,
  Hero,
  Node,
  RoomKind,
} from '@/protocol/generated'
import { CustomCardFrame } from './CustomCardFrame'
import { DelveOffer } from './DelveOffer'
import { POLL_MS, deckOptions, errorText } from './words'
import type { DeckOption } from './words'

const ROOM: Record<RoomKind, string> = {
  boss: 'Boss',
  treasure: 'Treasure',
  shop: 'Shop',
  event: 'Event',
  campfire: 'Campfire',
}

const BRAINS: Brain[] = ['Easy', 'Normal', 'Hard', 'VeryHard']

function bossOf(content: DelveContent, id: string | null | undefined): DelveBossView | undefined {
  return content.bosses.find((b) => b.id === id)
}

/** The boss's card and its phases, with the share of its life at which each begins. */
export function BossPanel({ content, boss }: { content: DelveContent; boss: DelveBossView }) {
  const card = content.cards.find((c) => c.id === boss.card)
  return (
    <section aria-label={`Boss: ${boss.name}`} className="grid gap-2">
      <p className="text-sm italic text-muted-foreground">“{boss.line}”</p>
      <div className="flex flex-wrap gap-3">
        {card ? <CustomCardFrame card={card} /> : null}
        {boss.phases.map(([id, percent]) => {
          const phase = content.cards.find((c) => c.id === id)
          return phase ? (
            <div key={id} className="grid gap-1">
              <span className="text-xs text-muted-foreground">At {percent}% of its life</span>
              <CustomCardFrame card={phase} />
            </div>
          ) : null
        })}
      </div>
    </section>
  )
}

function PartyPanel({ view, content }: { view: DelveView; content: DelveContent }) {
  const relic = (id: string) => content.cards.find((c) => c.id === id)?.name ?? id
  return (
    <section aria-label="Party" className="surface grid gap-2 p-3">
      <h2 className="flex items-center justify-between font-semibold">
        Party
        <span className="flex items-center gap-1 text-sm" aria-label={`${view.run.hearts} hearts left`}>
          <HeartIcon aria-hidden="true" className="size-4 text-destructive" />
          {view.run.hearts}
        </span>
      </h2>
      <ul className="grid gap-2">
        {view.run.heroes.map((hero, i) => (
          <li key={`${hero.name}-${i}`} className="grid gap-0.5 border-b pb-2 text-sm last:border-b-0">
            <span className="font-medium">
              {hero.name}
              {hero.bot ? <span className="text-muted-foreground"> (bot, {hero.bot})</span> : null}
              {i === view.me ? <span className="text-muted-foreground"> (you)</span> : null}
            </span>
            <span className="text-muted-foreground">
              {hero.deck?.name ?? 'No deck yet'} · {hero.life} life · {hero.gold} gold
            </span>
            {hero.relics.length > 0 ? (
              <span className="text-xs">Relics: {hero.relics.map(relic).join(', ')}</span>
            ) : null}
            {hero.cards.length > 0 ? (
              <span className="text-xs">Won cards: {hero.cards.join(', ')}</span>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  )
}

function Lobby({
  view,
  act,
  busy,
  decks,
}: {
  view: DelveView
  act: (action: DelveAction) => void
  busy: boolean
  decks: DeckOption[]
}) {
  const [level, setLevel] = useState<Brain>('Normal')
  const mayChoose = (hero: Hero, i: number) => i === view.me || (view.host && hero.bot !== null)
  const ready = view.run.heroes.every((h) => h.deck !== null)
  return (
    <section aria-label="Lobby" className="surface grid gap-3 p-4">
      <h2 className="text-lg font-semibold">Gather the party</h2>
      <p className="text-sm text-muted-foreground">
        Share the invite code <strong className="font-mono">{view.code}</strong>. Up to six heroes; bot
        allies fill the gaps.
      </p>
      <ul className="grid gap-2">
        {view.run.heroes.map((hero, i) => (
          <li key={`${hero.name}-${i}`} className="flex flex-wrap items-center gap-2">
            <span className="w-32 font-medium">{hero.name}</span>
            {mayChoose(hero, i) ? (
              <NativeSelect
                aria-label={`Deck for ${hero.name}`}
                value={hero.deck?.id ?? ''}
                disabled={busy}
                onChange={(e) => act({ kind: 'choose_deck', hero: i, deck: e.target.value })}
              >
                <NativeSelectOption value="">Choose a Commander deck…</NativeSelectOption>
                {decks.map((d) => (
                  <NativeSelectOption key={d.value} value={d.value}>
                    {d.label}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            ) : (
              <span className="text-sm text-muted-foreground">{hero.deck?.name ?? 'Choosing a deck…'}</span>
            )}
            {view.host && i > 0 ? (
              <Button
                size="sm"
                variant="ghost"
                disabled={busy}
                onClick={() => act({ kind: 'remove', hero: i })}
              >
                Remove
              </Button>
            ) : null}
          </li>
        ))}
      </ul>
      {view.host ? (
        <div className="flex flex-wrap items-center gap-2">
          <NativeSelect
            aria-label="Bot ally level"
            value={level}
            onChange={(e) => setLevel(e.target.value as Brain)}
          >
            {BRAINS.map((b) => (
              <NativeSelectOption key={b} value={b}>
                {b === 'VeryHard' ? 'Very hard' : b}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <Button
            variant="outline"
            disabled={busy || view.run.heroes.length >= 6}
            onClick={() => act({ kind: 'add_bot', level })}
          >
            Add a bot ally
          </Button>
          <Button disabled={busy || !ready} onClick={() => act({ kind: 'start' })}>
            Descend
          </Button>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          The host leads the party in once every hero has a deck.
        </p>
      )}
    </section>
  )
}

/** The ladder: bosses and the camps between them; the party's next choices take votes. */
export function Ladder({
  map,
  view,
  content,
  act,
  busy,
}: {
  map: FloorMap
  view: DelveView
  content: DelveContent
  act: (action: DelveAction) => void
  busy: boolean
}) {
  const run = view.run
  const choices = run.status === 'Map' ? (run.at === null ? [] : (map.nodes[run.at]?.next ?? [])) : []
  const rows: Node[][] = []
  for (const node of map.nodes) (rows[node.row] ??= []).push(node)
  const voters = (node: number) =>
    Object.entries(run.votes)
      .filter(([, v]) => v === node)
      .map(([hero]) => run.heroes[Number(hero)]?.name ?? hero)
  const torch = run.heroes.length > 0 ? run.cleared.length % run.heroes.length : 0
  return (
    <section aria-label="Ladder" className="surface grid gap-2 p-4">
      <h2 className="text-lg font-semibold">The ladder</h2>
      {run.status === 'Map' ? (
        <p className="text-sm text-muted-foreground">
          Vote for the next camp. A tie goes to the torch bearer, {run.heroes[torch]?.name}.
        </p>
      ) : null}
      <ol className="grid gap-2">
        {rows.map((row, r) => (
          <li key={r} className="flex flex-wrap gap-2">
            {row.map((node) => {
              const here = run.at === node.id
              const done = run.cleared.includes(node.id)
              const open = choices.includes(node.id)
              const boss = bossOf(content, node.boss)
              const label = boss ? boss.name : ROOM[node.kind]
              const votes = voters(node.id)
              return (
                <div
                  key={node.id}
                  data-node={node.id}
                  data-current={here || undefined}
                  className={cn(
                    'flex min-w-40 items-center gap-2 rounded-lg border px-3 py-2 text-sm',
                    here && 'border-primary ring-2 ring-primary/40',
                    done && 'opacity-60',
                  )}
                >
                  {node.kind === 'boss' ? (
                    <SkullIcon aria-hidden="true" className="size-4" />
                  ) : (
                    <FlameIcon aria-hidden="true" className="size-4" />
                  )}
                  <span>{label}</span>
                  {done ? <Pill>Cleared</Pill> : null}
                  {votes.length > 0 ? (
                    <span className="text-xs text-muted-foreground">({votes.join(', ')})</span>
                  ) : null}
                  {open && view.me !== undefined ? (
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busy}
                      onClick={() => act({ kind: 'vote', node: node.id })}
                    >
                      Vote
                    </Button>
                  ) : null}
                </div>
              )
            })}
          </li>
        ))}
      </ol>
    </section>
  )
}

function FightPanel({ view, content }: { view: DelveView; content: DelveContent }) {
  const status = view.run.status
  const game = typeof status === 'object' && 'Fight' in status ? status.Fight.game : null
  const boss = bossOf(content, view.boss)
  return (
    <section aria-label="Boss fight" className="surface grid gap-3 p-4">
      <h2 className="flex items-center gap-2 text-lg font-semibold">
        <SwordsIcon aria-hidden="true" className="size-5" />
        {boss ? boss.name : 'A boss'} awaits
      </h2>
      {game ? (
        <Link
          to="/game/$gameId"
          params={{ gameId: game }}
          className={cn(buttonVariants(), 'justify-self-start')}
        >
          Go to the fight
        </Link>
      ) : (
        <p className="text-sm text-muted-foreground">The fight is being prepared…</p>
      )}
      {boss ? <BossPanel content={content} boss={boss} /> : null}
    </section>
  )
}

function EndPanel({ view }: { view: DelveView }) {
  const status = view.run.status
  const won = status === 'Victory'
  return (
    <section aria-label="The end" className="surface grid gap-2 p-4">
      <h2 className="flex items-center gap-2 text-lg font-semibold">
        {won ? (
          <TrophyIcon aria-hidden="true" className="size-5" />
        ) : (
          <ShieldIcon aria-hidden="true" className="size-5" />
        )}
        {won ? 'Victory!' : status === 'Defeat' ? 'The party fell' : 'The party turned back'}
      </h2>
      <p className="text-sm text-muted-foreground">
        Seed code <strong className="font-mono">{view.seed}</strong> replays this ladder for another party.
      </p>
    </section>
  )
}

function RoomPanel({
  view,
  content,
  act,
  busy,
}: {
  view: DelveView
  content: DelveContent
  act: (action: DelveAction) => void
  busy: boolean
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
          content={content}
          busy={busy}
          onPick={(pick) => act({ kind: 'pick', pick })}
        />
      ) : null}
      {waiting.length > 0 && !offer ? (
        <p role="status" className="text-sm text-muted-foreground">
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
  loadDecks = deckOptions,
}: {
  id: string
  client?: DelvesApi
  loadDecks?: () => Promise<DeckOption[]>
}) {
  usePageTitle('Delve')
  const { view, content, error, busy, act } = useDelve(client, id)
  const [decks, setDecks] = useState<DeckOption[]>([])
  useEffect(() => {
    let live = true
    loadDecks()
      .then((list) => live && setDecks(list))
      .catch(() => live && setDecks([]))
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
      <PageShell width="page">
        <Loading label="Loading the delve…">
          <Skeleton className="h-40" />
        </Loading>
      </PageShell>
    )
  }
  const status = view.run.status
  const fighting = typeof status === 'object' && 'Fight' in status
  const over = status === 'Victory' || status === 'Defeat' || status === 'Abandoned'
  return (
    <PageShell width="wide" className="grid gap-4 sm:py-6">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{content.theme}</h1>
        <span className="text-sm text-muted-foreground">
          Invite <span className="font-mono">{view.code}</span> · Seed{' '}
          <span className="font-mono">{view.seed}</span> · Heat {view.run.heat}
        </span>
      </div>
      {error !== null ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
      <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
        <div className="grid content-start gap-4">
          {status === 'Lobby' ? <Lobby view={view} act={act} busy={busy} decks={decks} /> : null}
          {fighting ? <FightPanel view={view} content={content} /> : null}
          {status === 'Room' ? <RoomPanel view={view} content={content} act={act} busy={busy} /> : null}
          {over ? <EndPanel view={view} /> : null}
          {status !== 'Lobby' ? (
            <Ladder map={view.run.map} view={view} content={content} act={act} busy={busy} />
          ) : (
            <section aria-label="The Warrens" className="surface p-4 text-sm text-muted-foreground">
              {content.text}
            </section>
          )}
        </div>
        <aside className="grid content-start gap-4">
          <PartyPanel view={view} content={content} />
          <section aria-label="Log" className="surface grid gap-1 p-3 text-sm">
            <h2 className="font-semibold">Log</h2>
            <ol className="grid gap-0.5 text-muted-foreground">
              {view.run.log.slice(-8).map((line, i) => (
                <li key={`${i}-${line}`}>{line}</li>
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

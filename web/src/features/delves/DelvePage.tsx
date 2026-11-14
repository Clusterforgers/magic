/**
 * `/delves/$delveId`: one run, in the dungeon's look. The run header (Heat, hearts, seed, the party's life and relics),
 * then the stage of the run: the lobby with the deck picker, the boss fight (its portrait, card, phases and a link to
 * the table), the camp scenes, or the end; then the descent with the votes on the next camp. The page reads the run
 * every few seconds, which also reads a finished fight back.
 */
import { Link } from '@tanstack/react-router'
import { CircleAlertIcon, ShieldIcon, TrophyIcon } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Loading } from '@/components/Loading'
import { PageShell } from '@/components/PageShell'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button, buttonVariants } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Skeleton } from '@/components/ui/skeleton'
import { usePageTitle } from '@/hooks/usePageTitle'
import { delvesApi } from '@/net/delves'
import type { DelvesApi } from '@/net/delves'
import { cn } from '@/lib/utils'
import type { DelveAction, DelveContent, DelveView } from '@/protocol/generated'
import { BossLadder } from './BossLadder'
import { DelveOffer } from './DelveOffer'
import { loadPickerDecks, notePlayed, readPlayed } from './deckPicker'
import type { LastPlayed, PickerDecks } from './deckPicker'
import { BossIntro } from './BossIntro'
import { DeckPicker } from './DeckPicker'
import { lastFight, nextStep } from './model'
import type { FindCard } from './findCard'
import { PartyLobby } from './PartyLobby'
import { RunHeader } from './RunHeader'
import { POLL_MS, errorText } from './words'
import { RevivalNote } from './DelvesRules'

function FightPanel({
  view,
  content,
  onChooseDeck,
}: {
  view: DelveView
  content: DelveContent
  /** Opens the deck picker for the caller's hero, whose deck no longer plays. */
  onChooseDeck: () => void
}) {
  const status = view.run.status
  const game = typeof status === 'object' && 'Fight' in status ? status.Fight.game : null
  const mine = view.deck_problems.find((problem) => problem.hero === view.me)
  const waiting = view.deck_problems
    .filter((problem) => problem.hero !== view.me)
    .map((problem) => view.run.heroes[problem.hero]?.name ?? `Hero ${problem.hero + 1}`)
  return (
    <BossIntro view={view} content={content}>
      <div className="grid gap-2">
        {lastFight(view.run) === 'lost' ? (
          <p role="status" className="rounded-lg border border-destructive/50 bg-surface p-3 text-sm">
            The party was beaten back and lost a heart ({view.run.hearts} left). Every hero regroups at half
            life for another try.
          </p>
        ) : null}
        {game !== null ? (
          <Link
            to="/game/$gameId"
            params={{ gameId: game }}
            className={cn(buttonVariants({ size: 'lg' }), 'w-full sm:w-auto sm:justify-self-start')}
          >
            Enter the fight
          </Link>
        ) : mine !== undefined ? (
          <div className="grid gap-2 rounded-lg border border-destructive/50 bg-surface p-3 text-sm">
            <p>Your deck can no longer be played ({mine.reason}). Choose another to fight this boss.</p>
            <Button className="justify-self-start" onClick={onChooseDeck}>
              Choose another deck
            </Button>
          </div>
        ) : waiting.length > 0 ? (
          <p role="status" className="text-sm text-muted-foreground">
            Waiting for {waiting.join(', ')} to choose another deck.
          </p>
        ) : (
          <p role="status" className="text-sm text-muted-foreground">
            The fight is being prepared…
          </p>
        )}
      </div>
      <RevivalNote />
    </BossIntro>
  )
}

function EndPanel({ view, content }: { view: DelveView; content: DelveContent }) {
  const status = view.run.status
  const won = status === 'Victory'
  const Icon = won ? TrophyIcon : ShieldIcon
  const bosses = view.run.map.nodes.filter((node) => node.kind === 'boss')
  const cleared = bosses.filter((node) => view.run.cleared.includes(node.id)).length
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
      <dl aria-label="Run summary" className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm sm:grid-cols-4">
        <dt className="text-muted-foreground">Bosses</dt>
        <dd className="font-semibold">
          {cleared} of {bosses.length}
        </dd>
        <dt className="text-muted-foreground">Hearts left</dt>
        <dd className="font-semibold">{view.run.hearts}</dd>
        <dt className="text-muted-foreground">Heat</dt>
        <dd className="font-semibold">{view.run.heat}</dd>
        <dt className="text-muted-foreground">Rooms</dt>
        <dd className="font-semibold">{view.run.cleared.length}</dd>
      </dl>
      <ul aria-label="Heroes" className="grid w-full max-w-prose gap-1 text-left text-sm">
        {view.run.heroes.map((hero, at) => (
          <li key={`${at}-${hero.name}`} className="flex flex-wrap justify-between gap-2 border-b py-1">
            <span className="font-medium">{hero.name}</span>
            <span className="text-muted-foreground">
              {hero.life} life · {hero.gold} gold · {hero.relics.length} relics
              {hero.relics.length > 0
                ? ` (${hero.relics.map((id) => content.cards.find((c) => c.id === id)?.name ?? id).join(', ')})`
                : ''}
            </span>
          </li>
        ))}
      </ul>
      <p className="max-w-prose text-sm text-muted-foreground">
        Seed code <strong className="font-mono text-foreground">{view.seed}</strong> replays this ladder for
        another party.
      </p>
      <div className="flex flex-wrap justify-center gap-2">
        <CopyButton text={view.seed} label="Copy seed code">
          Copy seed code
        </CopyButton>
        <Link to="/delves" className={buttonVariants({ variant: 'outline' })}>
          Back to Delves
        </Link>
      </div>
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
  const boss = content.bosses.find((entry) => entry.id === view.boss)
  return (
    <div className="grid gap-3">
      {lastFight(view.run) === 'won' ? (
        <p
          role="status"
          className="flex items-center gap-2 rounded-xl border-2 border-relic bg-delve-stone p-3 text-sm font-medium"
        >
          <TrophyIcon aria-hidden="true" className="size-5 text-relic" />
          {boss ? `${boss.name} falls!` : 'The boss falls!'} Each hero heals 10, takes 50 gold and picks loot.
        </p>
      ) : null}
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
          Waiting for {waiting.join(', ')} to pick. Then the party votes for the next room on the descent
          below.
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
  const [replacing, setReplacing] = useState(false)
  const [ending, setEnding] = useState(false)
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
  const step = nextStep(view)
  const me = view.me === undefined ? undefined : view.run.heroes[view.me]
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
          {step === '' || fighting ? null : (
            <p data-slot="next-step" className="text-sm text-muted-foreground">
              {step}
            </p>
          )}
          {fighting ? (
            <FightPanel view={view} content={content} onChooseDeck={() => setReplacing(true)} />
          ) : null}
          <Dialog
            open={fighting && replacing && me !== undefined}
            onOpenChange={(open) => !open && setReplacing(false)}
          >
            <DialogContent className="max-w-6xl p-4 sm:p-6">
              <DialogTitle>Choose a deck for {me?.name}</DialogTitle>
              <DialogDescription>
                Your deck can no longer be played. The new one keeps the cards your rewards added.
              </DialogDescription>
              {me === undefined ? null : (
                <DeckPicker
                  heroName={me.name}
                  entries={decks?.entries ?? null}
                  groups={decks?.groups ?? []}
                  now={Date.now()}
                  current={me.deck?.id ?? null}
                  played={played}
                  onCancel={() => setReplacing(false)}
                  onTake={(deck) => {
                    act({ kind: 'choose_deck', hero: view.me ?? 0, deck })
                    setPlayed(notePlayed(deck, new Date()))
                    setReplacing(false)
                  }}
                />
              )}
            </DialogContent>
          </Dialog>
          {status === 'Room' ? (
            <RoomPanel view={view} content={content} act={act} busy={busy} find={find} />
          ) : null}
          {over ? <EndPanel view={view} content={content} /> : null}
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
            <>
              <Button variant="ghost" disabled={busy} onClick={() => setEnding(true)}>
                End the run
              </Button>
              <AlertDialog open={ending} onOpenChange={setEnding}>
                <AlertDialogContent>
                  <AlertDialogTitle>End the run?</AlertDialogTitle>
                  <AlertDialogDescription>
                    The party turns back for good and the run leaves everyone's list. This cannot be undone.
                  </AlertDialogDescription>
                  <AlertDialogFooter>
                    <Button variant="outline" onClick={() => setEnding(false)}>
                      Keep going
                    </Button>
                    <Button
                      variant="destructive"
                      onClick={() => {
                        setEnding(false)
                        act({ kind: 'abandon' })
                      }}
                    >
                      End the run
                    </Button>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </>
          ) : null}
        </aside>
      </div>
    </PageShell>
  )
}

import { CircleAlertIcon, EyeIcon } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useDebouncedValue } from '@/hooks/useDebouncedValue'
import { useMediaQuery } from '@/hooks/useMediaQuery'
import type { GameFormat } from '@/lib/houseRules'
import type { DecksApi } from '@/net/decks'
import type { PreconsApi } from '@/net/precons'
import type { SeatDeck } from '@/protocol/generated'
import type { DeckEntry } from './deckEntries'
import { deckWords, issueWords, NO_ISSUES } from './deckWords'
import { DeckPane } from './DeckPane'
import { BuiltInSource, OwnSource, PreconSource } from './DeckSources'
import type { DeckChoice } from './useDeckChoice'
import { useDeckDetails } from './useDeckDetail'
import type { LobbyApi } from './useLobbyControls'
import { useVerdicts } from './useVerdicts'

/** The width from which the preview sits beside the list; below it opens as a dialog. */
export const WIDE_QUERY = '(min-width: 1000px)'

/** How long a selection rests before the deck's cards and its verdict are asked for. */
const REST_MS = 200

/** What the chooser asks of the page. */
export interface DeckChooserProps {
  /** The built-in decks come from here. */
  api: Pick<LobbyApi, 'listDecks'>
  /** The signed-in account's decks come from here, and every deck's cards and verdict at the table. */
  decks: DecksApi
  /** The precons and their cards come from here. */
  precons: Pick<PreconsApi, 'list' | 'get'>
  /** The game the decks are judged for. */
  game: string
  /** The format of the game: it decides which decks can be taken at all. */
  format: GameFormat
  /** A signed-in visitor has stored decks to choose among; a guest has the built-in decks and the precons. */
  signedIn: boolean
  /** The deck the roster shows for this seat: null for none, undefined while no roster has said. */
  current: SeatDeck | null | undefined
  /** This seat pressed Ready, so its deck cannot change. */
  ready: boolean
  /** Some request of the lobby is out, so nothing else can be sent. */
  busy: boolean
  choice: DeckChoice
  /** Told which deck is selected but not chosen, and why this table cannot take it, for the Ready button's words. */
  onPreview?(previewing: { name: string; blocked: string | null } | null): void
}

type Kind = 'builtin' | 'own' | 'precons'

/** The kinds of deck a game of this format and this visitor can choose among, in the order of the tabs. */
function deckKinds(format: GameFormat, signedIn: boolean): { kind: Kind; label: string }[] {
  return [
    ...(signedIn ? [{ kind: 'own' as const, label: 'Your decks' }] : []),
    ...(format === 'normal' ? [{ kind: 'builtin' as const, label: 'Built-in' }] : []),
    { kind: 'precons' as const, label: 'Precons' },
  ]
}

/**
 * The player's own deck: what the table is told of it and, until the seat is ready, the decks to choose among in tabs
 * (the account's stored decks, built-in decks in Normal games, precons). Each tab is a list and the preview of the
 * selected deck beside it (a dialog under 1000 px): its commanders, legality at this table, support summary, mana
 * curve and every card; "Use this deck" (or Enter or a double click on a row) takes it. A deck the table cannot take
 * is shown but not chosen. The choice shows at once with a spinner; a refusal puts the old choice back and says why.
 */
export function DeckChooser({
  api,
  decks,
  precons,
  game,
  format,
  signedIn,
  current,
  ready,
  busy,
  choice,
  onPreview,
}: DeckChooserProps) {
  const { problem } = choice
  const kinds = deckKinds(format, signedIn)
  const first = kinds[0]?.kind ?? 'precons'
  const [tab, setTab] = useState<Kind>(first)
  // A list is fetched when its tab is first opened, then kept.
  const [opened, setOpened] = useState<ReadonlySet<Kind>>(new Set([first]))
  const shown = kinds.some((entry) => entry.kind === tab) ? tab : first
  const [selected, setSelected] = useState<DeckEntry | null>(null)
  const [sheet, setSheet] = useState(false)
  const wide = useMediaQuery(WIDE_QUERY, true)
  const root = useRef<HTMLElement>(null)
  const watching = wide || sheet || ready
  const restedId = useDebouncedValue(selected?.id ?? null, REST_MS)
  const rested = watching && selected !== null && selected.id === restedId ? selected : null
  const details = useDeckDetails(decks, precons, rested)
  const verdicts = useVerdicts(decks, game, rested === null || rested.kind === 'builtin' ? [] : [rested.id])
  const select = useCallback((entry: DeckEntry) => setSelected(entry), [])
  const use = (entry: DeckEntry): void => {
    choice.choose({ id: entry.id, name: entry.name })
    setSheet(false)
  }
  const pane = (locked: boolean) => (
    <DeckPane
      entry={selected}
      detail={details.of(selected)}
      verdict={selected === null ? undefined : verdicts.of.get(selected.id)}
      format={format}
      chosen={locked || (selected !== null && selected.id === choice.chosenId)}
      pending={selected !== null && choice.pending?.id === selected.id}
      waiting={busy}
      onUse={use}
      onRetryDetail={details.retry}
      onRetryVerdict={verdicts.retry}
    />
  )
  const known = selected !== null && current != null && selected.name === current.name
  const verdict = selected === null ? undefined : verdicts.of.get(selected.id)
  const reasons =
    verdict?.status === 'ready' && verdict.verdict.kind === 'blocked' ? verdict.verdict.reasons : null
  const blocked = reasons === null ? null : (reasons[0] ?? null)
  const previewName = selected === null || known || ready ? null : selected.name
  useEffect(() => {
    onPreview?.(previewName === null ? null : { name: previewName, blocked })
  }, [onPreview, previewName, blocked])
  const source = {
    game,
    format,
    current,
    busy,
    choice,
    selectedId: selected?.id ?? null,
    pictures: (id: string) => details.known(id)?.commanders ?? [],
    onSelect: select,
    onActivate: () => setSheet(!wide),
    onChoose: use,
  }
  return (
    <section
      ref={root}
      aria-labelledby="deck-title"
      data-slot="deck-chooser"
      className="flex min-h-0 flex-1 flex-col gap-3"
    >
      <div className="flex flex-wrap items-start justify-between gap-x-3 gap-y-1">
        <div className="grid min-w-0 gap-1">
          <h2 id="deck-title" className="text-lg font-semibold">
            Your deck
          </h2>
          <p data-slot="deck-current" className="text-sm [overflow-wrap:anywhere]">
            {current === undefined
              ? null
              : current === null
                ? 'You have no deck yet. Choose one to press Ready.'
                : deckWords(current)}
          </p>
        </div>
        {ready || current == null ? null : wide ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => root.current?.querySelector<HTMLElement>('[role="option"][tabindex="0"]')?.focus()}
          >
            Change
          </Button>
        ) : (
          <Button type="button" variant="outline" size="sm" disabled={!known} onClick={() => setSheet(true)}>
            <EyeIcon aria-hidden="true" />
            View deck
          </Button>
        )}
      </div>
      {ready ? (
        <>
          <p data-slot="deck-locked" className="text-sm text-muted-foreground">
            Your deck is locked while you are ready. Press Not ready to change it.
          </p>
          {wide && known ? (
            <div className="min-h-0 flex-1 overflow-y-auto" data-slot="deck-pane">
              {pane(true)}
            </div>
          ) : null}
        </>
      ) : (
        <div className="grid min-h-0 flex-1 gap-4 min-[1000px]:grid-cols-[minmax(16rem,20rem)_minmax(0,1fr)]">
          <Tabs
            value={shown}
            onValueChange={(value) => {
              const next = kinds.find((entry) => entry.kind === value)
              if (next === undefined) return
              setTab(next.kind)
              setSelected(null)
              setOpened((before) => new Set(before).add(next.kind))
            }}
            className="flex min-h-0 flex-col gap-3"
          >
            <TabsList aria-label="Kinds of deck" className="max-w-full">
              {kinds.map((entry) => (
                <TabsTrigger key={entry.kind} value={entry.kind}>
                  {entry.label}
                </TabsTrigger>
              ))}
            </TabsList>
            {kinds.map((entry) => (
              <TabsContent
                key={entry.kind}
                value={entry.kind}
                keepMounted
                className="flex min-h-0 flex-1 flex-col outline-none"
              >
                {!opened.has(entry.kind) ? null : entry.kind === 'builtin' ? (
                  <BuiltInSource api={api} active={shown === entry.kind} {...source} />
                ) : entry.kind === 'own' ? (
                  <OwnSource decks={decks} active={shown === entry.kind} {...source} />
                ) : (
                  <PreconSource precons={precons} active={shown === entry.kind} {...source} />
                )}
              </TabsContent>
            ))}
          </Tabs>
          {wide ? (
            <div
              data-slot="deck-pane"
              className="surface min-h-[18rem] min-w-0 overflow-y-auto overscroll-contain p-4"
            >
              {pane(false)}
            </div>
          ) : (
            <Dialog open={sheet} onOpenChange={setSheet}>
              <DialogContent data-slot="deck-sheet" className="max-w-xl p-4 sm:p-6">
                <DialogTitle className="sr-only">Deck preview</DialogTitle>
                <DialogDescription className="sr-only">
                  The deck's cards, how far the game plays them and whether this table takes it.
                </DialogDescription>
                {pane(false)}
              </DialogContent>
            </Dialog>
          )}
        </div>
      )}
      <p
        data-slot="deck-note"
        aria-live="polite"
        aria-atomic="true"
        className="text-sm text-muted-foreground empty:hidden"
      >
        {choice.note}
      </p>
      {problem === null ? null : (
        <Alert variant="destructive" data-slot="deck-problem" data-blocked={problem.blocked || undefined}>
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="grid gap-2 text-foreground">
            <span className="[overflow-wrap:anywhere]">{problem.words}</span>
            {problem.blocked ? (
              problem.issues.length === 0 ? (
                <span>{NO_ISSUES}</span>
              ) : (
                <ul aria-label="What breaks the rules" className="list-disc pl-4">
                  {problem.issues.map((issue, index) => (
                    <li
                      key={`${issue.code}-${issue.oracle_id ?? ''}-${index}`}
                      className="[overflow-wrap:anywhere]"
                    >
                      {issueWords(issue)}
                    </li>
                  ))}
                </ul>
              )
            ) : null}
          </AlertDescription>
        </Alert>
      )}
    </section>
  )
}

import { CircleAlertIcon } from 'lucide-react'
import { useState } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { GameFormat } from '@/lib/houseRules'
import type { DecksApi } from '@/net/decks'
import type { PreconsApi } from '@/net/precons'
import type { SeatDeck } from '@/protocol/generated'
import { BuiltInList, OwnList, PreconList } from './ChoiceLists'
import { deckWords, issueWords, NO_ISSUES } from './deckWords'
import type { DeckChoice } from './useDeckChoice'
import type { LobbyApi } from './useLobbyControls'

/** What the chooser asks of the page. */
export interface DeckChooserProps {
  /** The built-in decks come from here. */
  api: Pick<LobbyApi, 'listDecks'>
  /** The signed-in account's decks come from here, and every deck's verdict at the table. */
  decks: DecksApi
  /** The precons come from here. */
  precons: Pick<PreconsApi, 'list'>
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
 * The player's own deck: what the table is told of it, and until the seat is ready the decks to choose among in
 * three kinds, built-in decks (Normal games), the account's stored decks (signed in) and precons (searchable). The
 * format of the game filters them, and each deck shows its verdict at this table before it is chosen: the whole deck
 * runs, some cards are played by hand, or the table cannot take it, in which case it cannot be pressed. The choice
 * shows at once with a spinner on the deck pressed; a refusal puts the old choice back and says why.
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
}: DeckChooserProps) {
  const { problem } = choice
  const kinds = deckKinds(format, signedIn)
  // A signed-in player most likely brings a deck of their own, so the list opens on those; a guest starts on the first.
  const first = kinds[0]?.kind ?? 'precons'
  const start: Kind = first
  const [tab, setTab] = useState<Kind>(start)
  // A list is fetched and its decks are checked when its tab is first opened, then kept.
  const [opened, setOpened] = useState<ReadonlySet<Kind>>(new Set([start]))
  const shown = kinds.some((entry) => entry.kind === tab) ? tab : first
  const list = { game, format, current, busy, choice }
  return (
    <section aria-labelledby="deck-title" data-slot="deck-chooser" className="grid grid-cols-1 gap-3">
      <div className="grid gap-1">
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
      {ready ? (
        <p data-slot="deck-locked" className="text-sm text-muted-foreground">
          Your deck is locked while you are ready. Press Not ready to change it.
        </p>
      ) : (
        <Tabs
          value={shown}
          onValueChange={(value) => {
            const next = kinds.find((entry) => entry.kind === value)
            if (next === undefined) return
            setTab(next.kind)
            setOpened((before) => new Set(before).add(next.kind))
          }}
        >
          <TabsList aria-label="Kinds of deck" className="max-w-full">
            {kinds.map((entry) => (
              <TabsTrigger key={entry.kind} value={entry.kind}>
                {entry.label}
              </TabsTrigger>
            ))}
          </TabsList>
          {kinds.map((entry) => (
            <TabsContent key={entry.kind} value={entry.kind} keepMounted className="outline-none">
              <div className="max-h-96 overflow-y-auto pr-1" data-slot="deck-list">
                {!opened.has(entry.kind) ? null : entry.kind === 'builtin' ? (
                  <BuiltInList api={api} {...list} />
                ) : entry.kind === 'own' ? (
                  <OwnList decks={decks} {...list} />
                ) : (
                  <PreconList precons={precons} checks={decks} {...list} />
                )}
              </div>
            </TabsContent>
          ))}
        </Tabs>
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

import { Link } from '@tanstack/react-router'
import { CheckIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { DeckPreview, DeckPreviewSkeleton } from '@/features/decks/preview/DeckPreview'
import type { PreviewLegality } from '@/features/decks/preview/DeckPreview'
import { NotValidatedDialog } from '@/features/decks/preview/NotValidatedDialog'
import type { DeckPreviewData, PreviewCard } from '@/features/decks/preview/model'
import type { GameFormat } from '@/lib/houseRules'
import { deckFormat } from './deckWords'
import type { DeckEntry } from './deckEntries'
import type { DeckDetail } from './useDeckDetail'
import type { VerdictState } from './useVerdicts'
import { blockedLine } from './verdict'

interface DeckPaneProps {
  entry: DeckEntry | null
  detail: DeckDetail
  /** What this table says of the deck; undefined until asked. */
  verdict: VerdictState | undefined
  format: GameFormat
  /** The deck is this seat's. */
  chosen: boolean
  /** The choice of this deck is out. */
  pending: boolean
  /** Another request of the lobby is out. */
  waiting: boolean
  onUse(entry: DeckEntry): void
  onRetryDetail(): void
  onRetryVerdict(id: string): void
}

/** A stand-in for the deck while its cards load: the list's own facts, with the commanders it knows. */
function standIn(entry: DeckEntry): DeckPreviewData {
  const commanders: PreviewCard[] = entry.commanders.map((card) => ({
    oracleId: card.oracleId,
    name: card.name,
    count: 1,
    typeLine: 'Legendary Creature',
    manaValue: 0,
    manaCost: '',
    identity: entry.identity ?? [],
    support: 'verified',
    reasons: [],
  }))
  return {
    name: entry.name,
    source: entry.kind === 'precon' ? `Precon ${entry.set?.toUpperCase() ?? ''}`.trim() : 'Your deck',
    format: entry.format,
    commanders,
    cards: [],
    sideboard: [],
  }
}

function legalityOf(
  verdict: VerdictState | undefined,
  format: GameFormat,
  builtIn: boolean,
): PreviewLegality {
  const name = deckFormat(format)
  if (builtIn) return { verdict: 'legal', format: name }
  if (verdict === undefined || verdict.status === 'checking') return { verdict: 'checking', format: name }
  if (verdict.status === 'failed') return { verdict: 'unknown', format: name }
  if (verdict.verdict.kind === 'blocked') {
    return { verdict: 'illegal', format: name, reasons: verdict.verdict.reasons }
  }
  return { verdict: 'legal', format: name }
}

/**
 * The preview of the selected deck beside the list: the shared `DeckPreview` with this table's verdict as its
 * legality, a link to the cards played by hand, and "Use this deck", which a deck the table cannot take does not
 * allow (the reason stands beside it). The built-in decks have no card data and list their cards by name.
 */
export function DeckPane({
  entry,
  detail,
  verdict,
  format,
  chosen,
  pending,
  waiting,
  onUse,
  onRetryDetail,
  onRetryVerdict,
}: DeckPaneProps) {
  if (entry === null) {
    return (
      <p
        data-slot="deck-pane-empty"
        className="grid h-full place-items-center p-6 text-center text-sm text-muted-foreground"
      >
        Choose a deck in the list to see its cards.
      </p>
    )
  }
  const builtIn = entry.kind === 'builtin'
  const blocked = verdict?.status === 'ready' && verdict.verdict.kind === 'blocked' ? verdict.verdict : null
  const why = blocked === null ? null : blockedLine(blocked.reasons)
  const ready = detail.status === 'ready' ? detail.data : null
  const actions = (
    <>
      <Button
        type="button"
        data-slot="use-deck"
        loading={pending}
        disabled={chosen || blocked !== null || (waiting && !pending)}
        focusableWhenDisabled={blocked !== null}
        aria-describedby={why === null ? undefined : 'deck-pane-why'}
        onClick={() => onUse(entry)}
      >
        {chosen ? <CheckIcon aria-hidden="true" /> : null}
        {chosen ? 'This is your deck' : 'Use this deck'}
      </Button>
      {verdict?.status === 'failed' ? (
        <span className="flex flex-wrap items-center gap-2 text-xs">
          <span data-slot="deck-check-failed" className="text-muted-foreground">
            Could not check this deck at the table.
          </span>
          <Button type="button" variant="outline" size="sm" onClick={() => onRetryVerdict(entry.id)}>
            Check again
          </Button>
        </span>
      ) : null}
      {why === null ? null : (
        <p
          id="deck-pane-why"
          data-slot="deck-blocked"
          className="w-full text-xs text-destructive [overflow-wrap:anywhere]"
        >
          {why}
        </p>
      )}
    </>
  )
  const legality = legalityOf(verdict, format, builtIn)
  if (builtIn) {
    return (
      <DeckPreview
        deck={{ ...standIn(entry), source: 'Built-in deck' }}
        legality={legality}
        actions={actions}
        body={
          <ul
            aria-label={`Cards of ${entry.name}`}
            data-slot="builtin-cards"
            className="grid gap-1 text-sm sm:grid-cols-2"
          >
            {entry.listed.map((card) => (
              <li key={card.name} className="flex justify-between gap-2 border-b py-1">
                <span className="[overflow-wrap:anywhere]">{card.name}</span>
                <span className="tabular-nums text-muted-foreground">×{card.count}</span>
              </li>
            ))}
          </ul>
        }
      />
    )
  }
  return (
    <DeckPreview
      deck={ready ?? standIn(entry)}
      legality={legality}
      actions={actions}
      notValidated={
        ready === null ? undefined : entry.kind === 'own' ? (
          <Link
            to="/decks/$deckId/unvalidated"
            params={{ deckId: entry.id }}
            target="_blank"
            className="font-medium text-foreground underline underline-offset-3"
          >
            Not validated cards
          </Link>
        ) : (
          <NotValidatedDialog deck={ready} />
        )
      }
      body={
        ready !== null ? undefined : detail.status === 'failed' ? (
          <div className="grid justify-items-start gap-2" data-slot="deck-pane-failed">
            <p className="text-sm [overflow-wrap:anywhere]">{detail.message}</p>
            <Button type="button" variant="outline" size="sm" onClick={onRetryDetail}>
              Try again
            </Button>
          </div>
        ) : (
          <DeckPreviewSkeleton />
        )
      }
    />
  )
}

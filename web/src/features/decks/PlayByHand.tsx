import { HandIcon } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { BY_HAND_TITLE } from '@/lib/byHand'
import { playByHand } from './byHand'
import type { ByHandApi, ByHandDeck } from './byHand'
import { describeDeckError } from './errors'

type Marking =
  { status: 'idle' } | { status: 'saving' } | { status: 'done' } | { status: 'failed'; message: string }

/** "Play by hand in <deck>" for one deck, or the note that the owner plays the card by hand there. */
function DeckButton({
  deck,
  oracleId,
  name,
  api,
  onMarked,
}: {
  deck: ByHandDeck
  oracleId: string
  name: string
  api: ByHandApi
  onMarked?: (deckId: string) => void
}) {
  const [marking, setMarking] = useState<Marking>({ status: 'idle' })
  if (deck.byHand || marking.status === 'done') {
    return (
      <span
        data-slot="by-hand-done"
        className="inline-flex items-center gap-1 text-xs font-medium text-support-manual"
      >
        <HandIcon aria-hidden="true" className="size-4" />
        {`${BY_HAND_TITLE} in ${deck.name}`}
      </span>
    )
  }
  const mark = (): void => {
    setMarking({ status: 'saving' })
    playByHand(api, deck.id, oracleId).then(
      () => {
        setMarking({ status: 'done' })
        onMarked?.(deck.id)
      },
      (error: unknown) => setMarking({ status: 'failed', message: describeDeckError(error) }),
    )
  }
  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <Button
        type="button"
        variant="outline"
        size="xs"
        data-slot="play-by-hand"
        aria-label={`Play ${name} by hand in ${deck.name}`}
        disabled={marking.status === 'saving'}
        onClick={mark}
      >
        <HandIcon aria-hidden="true" />
        {`Play by hand in ${deck.name}`}
      </Button>
      {marking.status === 'failed' ? (
        <span role="alert" className="text-xs text-destructive">
          {marking.message}
        </span>
      ) : null}
    </span>
  )
}

/**
 * For a card that misbehaves: a "Play by hand in <deck>" button for each of the owner's decks that holds it, which
 * saves the deck with the card marked so the game hosts it as a manual card whatever its rules.
 */
export function PlayByHand({
  oracleId,
  name,
  decks,
  api,
  onMarked,
}: {
  oracleId: string
  name: string
  decks: readonly ByHandDeck[]
  api: ByHandApi
  /** Called once a deck is saved with the card marked. */
  onMarked?: (deckId: string) => void
}) {
  if (decks.length === 0) return null
  return (
    <div data-slot="play-by-hand-decks" className="flex flex-wrap items-center gap-x-3 gap-y-2">
      {decks.map((deck) => (
        <DeckButton key={deck.id} deck={deck} oracleId={oracleId} name={name} api={api} onMarked={onMarked} />
      ))}
    </div>
  )
}

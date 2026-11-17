import { ShuffleIcon } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { NativeSelect } from '@/components/ui/native-select'
import { RANDOM_MINE_NAME, RANDOM_PRECON, RANDOM_PRECON_NAME, randomMineId } from './randomDeck'
import type { DeckPick } from './useDeckChoice'

/** Props of {@link RandomDeckChoice}. */
export interface RandomDeckChoiceProps {
  /** A signed-in player can draw from their own decks; a guest only a precon. */
  signedIn: boolean
  /** The player's deck groups; with any, the draw from their decks can be narrowed to one. */
  groups: readonly { id: string; name: string }[]
  /** The id of the deck in force or asked for, so the draw in force reads as pressed. */
  current: string | null
  disabled: boolean
  onChoose(pick: DeckPick): void
}

/**
 * "Random precon" and "Random from my decks" (with a group to draw from when the player has groups): the server draws
 * a deck that suits the table, the lobby shows only the kind of draw, and the game tells every seat the deck at the
 * start.
 */
export function RandomDeckChoice({ signedIn, groups, current, disabled, onChoose }: RandomDeckChoiceProps) {
  const [group, setGroup] = useState<string | null>(null)
  const known = group !== null && groups.some((each) => each.id === group) ? group : null
  const mine = randomMineId(known)
  return (
    <div
      role="group"
      aria-label="Random deck"
      data-slot="random-deck"
      className="flex flex-wrap items-center gap-2"
    >
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={disabled}
        aria-pressed={current === RANDOM_PRECON}
        onClick={() => onChoose({ id: RANDOM_PRECON, name: RANDOM_PRECON_NAME })}
      >
        <ShuffleIcon aria-hidden="true" />
        Random precon
      </Button>
      {signedIn ? (
        <>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={disabled}
            aria-pressed={current?.startsWith(randomMineId(null)) === true}
            onClick={() => onChoose({ id: mine, name: RANDOM_MINE_NAME })}
          >
            <ShuffleIcon aria-hidden="true" />
            Random from my decks
          </Button>
          {groups.length === 0 ? null : (
            <NativeSelect
              size="sm"
              aria-label="Draw from"
              value={known ?? ''}
              disabled={disabled}
              onChange={(event) => setGroup(event.target.value === '' ? null : event.target.value)}
            >
              <option value="">All my decks</option>
              {groups.map((each) => (
                <option key={each.id} value={each.id}>
                  {each.name}
                </option>
              ))}
            </NativeSelect>
          )}
        </>
      ) : null}
    </div>
  )
}

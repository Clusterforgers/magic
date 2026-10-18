import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { KeyboardEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { powerToughness } from '../../cardModel'
import { blocksAnswer, useBlockPreview } from '../../combat/forecast'
import { usePublishPending } from '../../combat/pending'
import { usePreviewRequest } from '../../combat/usePreviewRequest'
import { useSeatNamer } from '../../names'
import { NO_IDS } from '../../selectors'
import { blockersByAttacker, blockSummary, uniqueNames } from '../combatText'
import { markCards, useCardMarks, useCardNotes } from '../marks'
import type { CardNotes } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PreviewOutcome } from '../PreviewOutcome'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'

const focusCard = (id: ObjectId): void =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()

interface PickerProps {
  /** The blocker being placed, as the prompt names it. */
  blocker: string
  /** The attackers it may block, as buttons. */
  choices: readonly { id: ObjectId; label: string; blocked: boolean }[]
  /** Whether it blocks something already, so the block can be taken back. */
  paired: boolean
  /** Changes with the blocker, which moves focus to the first button again. */
  focusKey: ObjectId
  onPick(attacker: ObjectId): void
  onRemove(): void
  onCancel(): void
}

/**
 * Which attacker a chosen blocker blocks: one button per attacker it may block, so an attacker on a seat that is not
 * drawn can still be picked. Focus moves to the first button on open, and Escape cancels.
 */
function AttackerPicker({ blocker, choices, paired, focusKey, onPick, onRemove, onCancel }: PickerProps) {
  const first = useRef<HTMLButtonElement>(null)
  useEffect(() => first.current?.focus(), [focusKey])
  return (
    <div
      role="group"
      aria-label={`Attackers ${blocker} may block`}
      data-slot="attacker-picker"
      className="grid basis-full gap-2"
      onKeyDown={(event: KeyboardEvent) => {
        if (event.key !== 'Escape') return
        event.preventDefault()
        onCancel()
      }}
    >
      <span className="text-sm">{blocker} blocks</span>
      <div className="flex max-h-48 flex-wrap items-center gap-2 overflow-y-auto p-0.5">
        {choices.map((choice, index) => (
          <Button
            key={choice.id}
            ref={index === 0 ? first : undefined}
            variant="outline"
            className="h-10 px-3"
            aria-pressed={choice.blocked}
            onClick={() => onPick(choice.id)}
          >
            {choice.label}
          </Button>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {paired ? (
          <Button variant="ghost" className="h-10 px-3" onClick={onRemove}>
            Remove block
          </Button>
        ) : null}
        <Button variant="ghost" className="h-10 px-3" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  )
}

/**
 * DeclareBlockers: creatures that may block are legal, the rest of the hand and battlefield dimmed. Choose a blocker
 * (click, Enter or Space), then the attacker it blocks (an attacker card, or its button in the list that opens);
 * both cards say so, and a blocker can be chosen again to change or remove its block. Confirm sends the pairs, No
 * blocks an empty answer.
 */
export const DeclareBlockersPrompt: PromptComponent<'DeclareBlockers'> = ({ options }) => {
  const { id, answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const seatName = useSeatNamer()

  // A blocker that may block nothing cannot block; a creature listed twice counts once.
  const offered = useMemo(() => {
    const byId = new Map<ObjectId, readonly ObjectId[]>()
    for (const option of options.blockers) {
      if (option.can_block.length > 0 && !byId.has(option.object)) byId.set(option.object, option.can_block)
    }
    return byId
  }, [options.blockers])
  const legal = useMemo(() => [...offered.keys()], [offered])
  const attackers = useMemo(() => [...new Set([...offered.values()].flat())], [offered])
  // Attackers that are aimed elsewhere are dimmed while an attacker is being chosen, so the ones to click stand out.
  const inCombat = usePlay(useShallow((state) => state.view?.combat.attackers.map((a) => a.object) ?? NO_IDS))
  const attackersShown = useMemo(() => [...new Set([...attackers, ...inCombat])], [attackers, inCombat])
  const involved = useMemo(() => [...legal, ...attackers], [legal, attackers])
  const objects = useObjects(involved)
  const byId = useMemo(() => {
    const found = new Map<ObjectId, ObjectView>()
    for (const object of objects) if (object !== undefined) found.set(object.id, object)
    return found
  }, [objects])
  const blockerNames = useMemo(
    () => uniqueNames(legal.map((id) => ({ id, name: byId.get(id)?.name ?? 'a creature' }))),
    [legal, byId],
  )
  const attackerNames = useMemo(
    () => uniqueNames(attackers.map((id) => ({ id, name: byId.get(id)?.name ?? 'an attacker' }))),
    [attackers, byId],
  )

  // Each blocker blocks one attacker; the map keeps the order the blocks were made in.
  const [pairs, setPairs] = useState<ReadonlyMap<ObjectId, ObjectId>>(new Map())
  // The blocks are drawn as lines while they are only a choice, and they put the table's forecast out of date.
  usePublishPending({ blocks: pairs })
  // The engine is asked what the blocks would do each time they change; its forecast comes back through the seam.
  const blocksMade = useMemo(() => blocksAnswer(pairs), [pairs])
  const previewStatus = usePreviewRequest(id, blocksMade)
  const preview = useBlockPreview()
  const [picked] = selection.ids
  const choosing = picked !== undefined && offered.has(picked) ? picked : undefined
  const choices = useMemo(
    () => (choosing === undefined ? [] : (offered.get(choosing) ?? [])),
    [offered, choosing],
  )
  const blockerName = (id: ObjectId): string => blockerNames.get(id) ?? 'a creature'
  const attackerName = (id: ObjectId): string => attackerNames.get(id) ?? 'an attacker'
  const controllerOf = useCallback(
    (id: ObjectId): string => {
      const controller = byId.get(id)?.controller
      return controller === undefined ? 'an opponent' : seatName(controller)
    },
    [byId, seatName],
  )
  const attackerLabel = (id: ObjectId): string => {
    const object = byId.get(id)
    const stats = object === undefined ? null : powerToughness(object)
    return `${attackerName(id)}${stats === null ? '' : ` ${stats}`} (${controllerOf(id)})`
  }

  const marks = useMemo(() => {
    if (choosing === undefined) return markCards(own, legal, pairs.keys())
    return markCards([...own, ...attackersShown], [...legal, ...choices], [...pairs.keys(), choosing])
  }, [own, legal, attackersShown, choices, pairs, choosing])

  const notes = useMemo<CardNotes>(() => {
    const written = new Map<ObjectId, { label: string; words: string }>()
    for (const [blocker, attacker] of pairs) {
      const name = byId.get(attacker)?.name ?? 'an attacker'
      written.set(blocker, {
        label: `Blocks ${name}`,
        words: `blocking ${name}, ${controllerOf(attacker)}'s`,
      })
    }
    for (const [attacker, blockers] of blockersByAttacker(pairs)) {
      const names = blockers.map((id) => byId.get(id)?.name ?? 'a creature')
      const [only] = names
      written.set(attacker, {
        label: names.length === 1 && only !== undefined ? `Blocked by ${only}` : `Blocked by ${names.length}`,
        words: `blocked by ${names.join(' and ')}`,
      })
    }
    if (choosing !== undefined && !pairs.has(choosing)) {
      written.set(choosing, { label: 'Pick attacker', words: 'choose the attacker it blocks' })
    }
    return written
  }, [pairs, byId, choosing, controllerOf])

  const pair = (blocker: ObjectId, attacker: ObjectId): void => {
    setPairs((current) => {
      const next = new Map(current)
      if (current.get(blocker) === attacker) next.delete(blocker)
      else next.set(blocker, attacker)
      return next
    })
    selection.clear()
  }
  const unpair = (blocker: ObjectId): void => {
    setPairs((current) => {
      const next = new Map(current)
      next.delete(blocker)
      return next
    })
    selection.clear()
  }
  const choose = (id: ObjectId): void => {
    if (offered.has(id)) {
      if (selection.has(id)) selection.clear()
      else selection.set([id])
      return
    }
    if (choosing !== undefined && choices.includes(id)) pair(choosing, id)
  }
  useCardMarks(marks, choose, { toggles: true })
  useCardNotes(notes)

  const back = (blocker: ObjectId, action: () => void): void => {
    action()
    focusCard(blocker)
  }

  const text =
    legal.length === 0
      ? 'None of your creatures can block. Choose No blocks to go on.'
      : choosing !== undefined
        ? `Choose the attacker ${blockerName(choosing)} blocks.`
        : pairs.size === 0
          ? 'Choose a creature to block with, then the attacker it blocks, or choose No blocks.'
          : `${blockSummary(pairs.size, new Set(pairs.values()).size)}. Confirm the blocks, or choose another creature.`

  return (
    <PromptLayout text={text}>
      <Button
        className="h-10 px-4"
        disabled={pairs.size === 0}
        onClick={() => answer({ DeclareBlockers: [...pairs] })}
      >
        Confirm blocks
      </Button>
      <Button variant="outline" className="h-10 px-4" onClick={() => answer({ DeclareBlockers: [] })}>
        No blocks
      </Button>
      {choosing === undefined ? null : (
        <AttackerPicker
          blocker={blockerName(choosing)}
          focusKey={choosing}
          paired={pairs.has(choosing)}
          choices={choices.map((id) => ({
            id,
            blocked: pairs.get(choosing) === id,
            label: attackerLabel(id),
          }))}
          onPick={(attacker) => back(choosing, () => pair(choosing, attacker))}
          onRemove={() => back(choosing, () => unpair(choosing))}
          onCancel={() => back(choosing, selection.clear)}
        />
      )}
      {pairs.size === 0 ? null : (
        <ul
          aria-label="Your blocks"
          data-slot="block-pairs"
          className="grid max-h-44 basis-full gap-1 overflow-y-auto"
        >
          {[...pairs].map(([blocker, attacker]) => (
            <li key={blocker} className="flex items-center justify-between gap-2 text-sm">
              <span className="min-w-0">
                Your {blockerName(blocker)} blocks {controllerOf(attacker)}&apos;s {attackerName(attacker)}
              </span>
              <span className="flex shrink-0 gap-1">
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`Change the block of ${blockerName(blocker)}`}
                  onClick={() => selection.set([blocker])}
                >
                  Change
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={`Remove the block of ${blockerName(blocker)}`}
                  onClick={() => unpair(blocker)}
                >
                  Remove
                </Button>
              </span>
            </li>
          ))}
        </ul>
      )}
      {preview === null && pairs.size === 0 ? null : (
        <PreviewOutcome
          slot="block-outcome"
          heading={pairs.size === 0 ? 'If you do not block' : 'If you block like this'}
          forecast={preview}
          status={previewStatus}
        />
      )}
    </PromptLayout>
  )
}

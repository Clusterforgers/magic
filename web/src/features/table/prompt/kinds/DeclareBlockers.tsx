import { useCallback, useEffect, useMemo, useState } from 'react'
import { Check, Shield } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { powerToughness } from '../../cardModel'
import { blocksAnswer, useBlockPreview } from '../../combat/forecast'
import { usePublishPending } from '../../combat/pending'
import { usePreviewRequest } from '../../combat/usePreviewRequest'
import { useSeatNamer } from '../../names'
import { NO_IDS } from '../../selectors'
import { blockersByAttacker, blockersNeeded, blockSummary, uniqueNames } from '../combatText'
import { BarButton } from '../CombatBar'
import { markCards, useCardMarks, useCardNotes } from '../marks'
import type { CardNotes } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import { PreviewOutcome } from '../PreviewOutcome'
import type { PromptComponent } from '../types'
import { usePrimaryKey } from '../usePrimaryKey'
import { useObjects } from '../useCombatNames'

/**
 * DeclareBlockers: creatures that may block are legal, the rest of the hand and battlefield dimmed. A click on a
 * blocker chooses it (one click blocks when it can block a single attacker), a click on an attacker then gives it the
 * block, a click on a blocker that blocks takes the block back, and another blocker chosen and the same attacker
 * clicked make a double block. The arrows say who blocks whom; no label is written on a card, except what an attacker
 * that needs several blockers still lacks. Confirm sends the pairs, No blocks an empty answer.
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
  // Attackers that need several blockers (menace) are blocked by that many or by none.
  const minimums = useMemo(() => {
    const needed = new Map<ObjectId, number>()
    for (const [attacker, count] of options.min_blockers ?? []) {
      if (count > 1 && attackers.includes(attacker)) needed.set(attacker, count)
    }
    return needed
  }, [options.min_blockers, attackers])
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
  // The attackers with some blockers but fewer than they need: Confirm waits for them.
  const blockerLists = useMemo(() => blockersByAttacker(pairs), [pairs])
  const short = useMemo(
    () =>
      [...minimums].filter(([attacker, needed]) => {
        const have = blockerLists.get(attacker)?.length ?? 0
        return have > 0 && have < needed
      }),
    [minimums, blockerLists],
  )
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
    const written = new Map<ObjectId, { label?: string; words: string; cue?: 'active' }>()
    for (const [blocker, attacker] of pairs) {
      written.set(blocker, {
        words: `blocking ${byId.get(attacker)?.name ?? 'an attacker'}, ${controllerOf(attacker)}'s`,
      })
    }
    for (const [attacker, blockers] of blockerLists) {
      const names = blockers.map((id) => byId.get(id)?.name ?? 'a creature')
      const lacking = (minimums.get(attacker) ?? 0) - blockers.length
      written.set(
        attacker,
        lacking > 0
          ? {
              label: `Needs ${lacking} more`,
              words: `blocked by ${names.join(' and ')}, needs ${lacking} more ${lacking === 1 ? 'blocker' : 'blockers'}`,
            }
          : { words: `blocked by ${names.join(' and ')}` },
      )
    }
    // An attacker that needs several blockers says so before anyone is put on it.
    for (const [attacker, needed] of minimums) {
      if (blockerLists.has(attacker)) continue
      written.set(attacker, { label: `Needs ${needed}+ blockers`, words: `needs ${blockersNeeded(needed)}` })
    }
    if (choosing !== undefined) {
      written.set(choosing, { words: 'choose the attacker it blocks', cue: 'active' })
    }
    return written
  }, [pairs, blockerLists, minimums, byId, choosing, controllerOf])

  const pair = (blocker: ObjectId, attacker: ObjectId): void => {
    setPairs((current) => new Map(current).set(blocker, attacker))
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
    const can = offered.get(id)
    if (can !== undefined) {
      const [only] = can
      if (pairs.has(id)) unpair(id)
      else if (choosing === id) selection.clear()
      else if (can.length === 1 && only !== undefined) pair(id, only)
      else selection.set([id])
      return
    }
    if (choosing !== undefined && choices.includes(id)) pair(choosing, id)
  }
  useCardMarks(marks, choose, { toggles: true })
  useCardNotes(notes)

  // Escape lets go of the blocker that was chosen.
  useEffect(() => {
    if (choosing === undefined) return undefined
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape' && !event.defaultPrevented) selection.clear()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [choosing, selection])

  const ready = pairs.size > 0 && short.length === 0
  const confirm = (): void => {
    if (ready) answer({ DeclareBlockers: [...pairs] })
  }
  // With nothing focused, Enter confirms the blocks that are chosen; Space is left alone.
  usePrimaryKey({ space: () => undefined, enter: confirm }, ready)

  const [firstShort] = short
  const text =
    legal.length === 0
      ? 'None of your creatures can block. Choose No blocks to go on.'
      : choosing !== undefined
        ? `Click the attacker ${blockerName(choosing)} blocks.`
        : firstShort !== undefined
          ? `${attackerName(firstShort[0])} needs ${blockersNeeded(firstShort[1])}. Choose another creature to block it, or take its block back.`
          : pairs.size === 0
            ? 'Click a blocker, then the attacker it blocks.'
            : `${blockSummary(pairs.size, new Set(pairs.values()).size)}.`

  return (
    <PromptLayout compact text={text}>
      {options.most === undefined ? null : (
        <p data-slot="declaration-limit" className="basis-full text-xs text-muted-foreground">
          No more than {options.most} {options.most === 1 ? 'creature' : 'creatures'} can block this combat.
        </p>
      )}
      {choosing === undefined ? null : (
        // An attacker whose seat is not drawn cannot be clicked, so it can be picked here too.
        <NativeSelect
          size="sm"
          className="max-w-44"
          aria-label={`Attacker ${blockerName(choosing)} blocks`}
          value=""
          onChange={(event) => {
            const attacker = Number(event.target.value)
            if (choices.includes(attacker)) pair(choosing, attacker)
          }}
        >
          <NativeSelectOption value="">Choose attacker…</NativeSelectOption>
          {choices.map((attacker) => (
            <NativeSelectOption key={attacker} value={attacker}>
              {attackerLabel(attacker)}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      )}
      <BarButton
        icon={Check}
        label="Confirm blocks"
        word="Block"
        keepWord
        disabled={!ready}
        onClick={confirm}
      />
      <BarButton
        variant="outline"
        icon={Shield}
        label="No blocks"
        word="No blocks"
        onClick={() => answer({ DeclareBlockers: [] })}
      />
      {minimums.size === 0 ? null : (
        <ul aria-label="Attackers that need several blockers" data-slot="block-minimums" className="sr-only">
          {[...minimums].map(([attacker, needed]) => {
            const have = blockerLists.get(attacker)?.length ?? 0
            return (
              <li key={attacker} data-short={(have > 0 && have < needed) || undefined}>
                {attackerLabel(attacker)} needs {blockersNeeded(needed)}
                {have > 0 && have < needed ? `: ${have} of ${needed} chosen` : ''}
              </li>
            )
          })}
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

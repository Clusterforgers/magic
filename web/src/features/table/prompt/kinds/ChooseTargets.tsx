import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, Target } from '@/protocol/generated'
import { useSeatNamer } from '../../names'
import { CardChoices } from '../CardChoices'
import { uniqueNames } from '../combatText'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { usePrompt } from '../PromptContext'
import { PromptLayout } from '../PromptLayout'
import {
  distinctTargets,
  fitsTotal,
  objectWords,
  sameTarget,
  subjectWords,
  targetKey,
  targetsSentence,
  totalWords,
} from '../targetText'
import type { PromptComponent } from '../types'
import { useObjects } from '../useCombatNames'

/**
 * ChooseTargets: the legal objects on the table are marked and a click, Enter or Space on one chooses it (again, takes
 * it back); every legal target is also offered under the sentence: a player as a button, an object as its card
 * picture (`CardChoices`), because a permanent may sit in a group, a graveyard, exile or the stack, where the table
 * draws no card to press. The chosen targets are listed with a button each to
 * take them back, Undo takes back the last one, and Confirm sends them once there are enough. A spell can be taken
 * back with Cancel, since nothing is paid yet; a triggered ability cannot, so it has no Cancel and waits for a target.
 * With a most for the total mana value the sentence tells the total so far, and a target that would go over it is
 * neither marked nor taken.
 */
export const ChooseTargetsPrompt: PromptComponent<'ChooseTargets'> = ({ options }) => {
  const { answer } = usePrompt()
  const own = useOwnCardIds()
  const seatName = useSeatNamer()
  const me = usePlay((state) => state.seat)
  const { min, max } = options
  const triggered = options.triggered === true

  const offered = useMemo(() => distinctTargets(options.legal), [options.legal])
  const objectIds = useMemo(
    () => offered.flatMap((target) => ('Object' in target ? [target.Object.id] : [])),
    [offered],
  )
  const known = useObjects(useMemo(() => [options.source, ...objectIds], [options.source, objectIds]))
  const source = known[0]
  const byId = useMemo(
    () => new Map(known.flatMap((object) => (object === undefined ? [] : [[object.id, object] as const]))),
    [known],
  )

  // Each legal target as its button names it; objects with the same name are numbered.
  const labels = useMemo(() => {
    const names = uniqueNames(objectIds.map((id) => ({ id, name: byId.get(id)?.name ?? 'an object' })))
    const words = new Map<string, string>()
    for (const target of offered) {
      if ('Player' in target) {
        const name = seatName(target.Player)
        words.set(targetKey(target), target.Player === me ? `${name} (you)` : name)
        continue
      }
      const { id } = target.Object
      const object = byId.get(id)
      const name = names.get(id) ?? 'an object'
      words.set(targetKey(target), object === undefined ? name : objectWords(object, name, seatName))
    }
    return words
  }, [offered, objectIds, byId, seatName, me])
  const labelOf = (target: Target): string => labels.get(targetKey(target)) ?? 'a target'
  const players = useMemo(() => offered.filter((target) => 'Player' in target), [offered])
  const cardLabels = useMemo(
    () =>
      new Map(
        offered.flatMap((target) =>
          'Object' in target
            ? [[target.Object.id, labels.get(targetKey(target)) ?? 'a target'] as const]
            : [],
        ),
      ),
    [offered, labels],
  )

  // The targets in the order they were chosen.
  const [chosen, setChosen] = useState<readonly Target[]>([])
  const has = (target: Target): boolean => chosen.some((picked) => sameTarget(picked, target))
  // With room for one a click on another target changes the choice; with room for several a full choice is closed.
  const closed = max > 1 && chosen.length >= max
  // A most for the total mana value: a target that would take the chosen ones over it cannot be added.
  const total = options.total ?? undefined
  const fits = (target: Target, current: readonly Target[]): boolean =>
    fitsTotal(options.legal, total, current, target)
  const pick = (target: Target): void =>
    setChosen((current) => {
      if (current.some((picked) => sameTarget(picked, target))) {
        return current.filter((picked) => !sameTarget(picked, target))
      }
      if (max === 1) return [target]
      if (!fits(target, current)) return current
      return current.length < max ? [...current, target] : current
    })

  const chosenIds = useMemo(
    () => chosen.flatMap((target) => ('Object' in target ? [target.Object.id] : [])),
    [chosen],
  )
  const fittingIds = useMemo(
    () =>
      offered.flatMap((target) =>
        'Object' in target && fitsTotal(options.legal, total, chosen, target) ? [target.Object.id] : [],
      ),
    [offered, options.legal, total, chosen],
  )
  const marks = useMemo(
    () => markCards(own, closed ? [] : fittingIds, chosenIds),
    [own, closed, fittingIds, chosenIds],
  )
  const onCard = (id: ObjectId): void => {
    const target = offered.find((candidate) => 'Object' in candidate && candidate.Object.id === id)
    if (target !== undefined) pick(target)
  }
  useCardMarks(marks, onCard, { toggles: true })

  const sentence = targetsSentence({
    subject: subjectWords(source?.name, triggered),
    wording: options.wording ?? '',
    min,
    max,
    chosen: chosen.length,
  })
  const totalText = totalWords(options.legal, total, chosen)
  const text = totalText === '' ? sentence : `${sentence} ${totalText}`
  const cancelWords = source?.zone === 'Battlefield' ? 'Cancel the ability' : 'Cancel the spell'

  return (
    <PromptLayout text={text}>
      <Button
        className="h-10 px-4"
        disabled={chosen.length < min}
        onClick={() => answer({ ChooseTargets: { Targets: [...chosen] } })}
      >
        Confirm targets
      </Button>
      <Button
        variant="outline"
        className="h-10 px-4"
        disabled={chosen.length === 0}
        onClick={() => setChosen((current) => current.slice(0, -1))}
      >
        Undo
      </Button>
      {options.can_cancel ? (
        <Button variant="outline" className="h-10 px-4" onClick={() => answer({ ChooseTargets: 'Cancel' })}>
          {cancelWords}
        </Button>
      ) : null}
      <CardChoices
        cards={objectIds}
        chosen={chosenIds}
        closed={closed}
        labels={cardLabels}
        objects={byId}
        onPress={onCard}
        label="Legal targets"
        className="max-h-[max(6rem,calc(var(--sheet-max,20rem)_-_12rem))]"
        lead={players.map((target) => {
          const picked = has(target)
          return (
            <Button
              key={targetKey(target)}
              size="sm"
              variant={picked ? 'default' : 'outline'}
              aria-pressed={picked}
              disabled={(closed || !fits(target, chosen)) && !picked}
              onClick={() => pick(target)}
            >
              {labelOf(target)}
            </Button>
          )
        })}
      />
      {chosen.length === 0 ? null : (
        <ul
          aria-label="Your targets"
          data-slot="chosen-targets"
          className="grid max-h-44 basis-full gap-1 overflow-y-auto"
        >
          {chosen.map((target) => (
            <li key={targetKey(target)} className="flex items-center justify-between gap-2 text-sm">
              <span className="min-w-0">{labelOf(target)}</span>
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Remove ${labelOf(target)}`}
                onClick={() => pick(target)}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
    </PromptLayout>
  )
}

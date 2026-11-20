import { anchor } from '@/features/learn/anchors'
import { Check } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { CHOSEN_BUTTON } from '../../ChosenMark'
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
import { UndoButton } from '../UndoButton'
import { PlayerFace } from '../PlayerFace'
import { QuickSelect } from '../QuickSelect'
import { useAllies } from '../useAllies'
import { QUICK_ROW_REM } from '../cardPrompts'
import { answersOnPress, WINDOW_FROM } from '../home'
import { useDrawnCards } from '../drawnCards'
import { useTargetTips } from '../targetTip'
import { usePublishDefenders } from '../../combat/defender'
import { quickButtons } from '../quickSelect'
import { useObjects } from '../useCombatNames'

/**
 * ChooseTargets: the legal objects on the table are marked and a click, Enter or Space on one chooses it (again, takes
 * it back); every legal target is also offered under the sentence: a player as a button, an object as its card
 * picture (`CardChoices`), because a permanent may sit in a group, a graveyard, exile or the stack, where the table
 * draws no card to press. The chosen targets are listed with a button each to
 * take them back, Undo takes back the last one, and Confirm sends them once there are enough. A spell or an ability
 * can be backed out of with its Undo button or Escape, since nothing is paid yet; a triggered ability cannot, so it has
 * none and waits for a target. With room for several the quick selection buttons (`QuickSelect`) add every legal
 * target, the viewer's own or their teammates', or take all back. With a most for the total mana value the sentence tells the total so far, and a
 * target that would go over it is neither marked nor taken.
 */
export const ChooseTargetsPrompt: PromptComponent<'ChooseTargets'> = ({ options }) => {
  const { answer, id: decisionId } = usePrompt()
  const own = useOwnCardIds()
  const seatName = useSeatNamer()
  const me = usePlay((state) => state.seat)
  const { min, max } = options
  // One target, or up to one, answers on the press: no Confirm (`answersOnPress`).
  const onPress = answersOnPress(options)
  const triggered = options.triggered === true

  const offered = useMemo(() => distinctTargets(options.legal), [options.legal])
  useTargetTips(decisionId, offered)
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
  // Targets on the table are pressed there; only the ones it does not draw (a graveyard, the stack, exile) are pictures here.
  const drawn = useDrawnCards(objectIds)
  // More than twelve are chosen in a window wherever they are (`detailPlace`).
  const offTable = useMemo(
    () => (objectIds.length > WINDOW_FROM ? objectIds : objectIds.filter((id) => !drawn.has(id))),
    [objectIds, drawn],
  )
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
  // The press of a target answers when it is the only one to choose; otherwise it is added to the choice.
  const choose = (target: Target): void => {
    if (onPress) answer({ ChooseTargets: { Targets: [target] } })
    else pick(target)
  }
  // A player target is pressed on its pod or tile too, which wear the legal mark and a "Target" tag (`promptBand.css`).
  const playerSeats = useMemo(
    () => players.flatMap((target) => ('Player' in target ? [target.Player] : [])),
    [players],
  )
  const lastPlayer = chosen.findLast((target) => 'Player' in target)
  usePublishDefenders(
    playerSeats,
    lastPlayer !== undefined && 'Player' in lastPlayer ? lastPlayer.Player : null,
    (seat) => choose({ Player: seat }),
  )
  // A Focus tile says how many legal targets its board holds that the table does not draw.
  const tileCounts = useMemo(() => {
    const counts = new Map<number, number>()
    for (const id of objectIds) {
      const seat = byId.get(id)?.controller
      if (seat === undefined || drawn.has(id) || byId.get(id)?.zone !== 'Battlefield') continue
      counts.set(seat, (counts.get(seat) ?? 0) + 1)
    }
    return counts
  }, [objectIds, byId, drawn])

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
    if (target !== undefined) choose(target)
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
  const allies = useAllies()
  const quick = quickButtons<Target>({
    legal: offered,
    chosen,
    max,
    same: sameTarget,
    seatOf: (target) =>
      'Player' in target ? target.Player : (byId.get(target.Object.id)?.controller ?? null),
    me,
    allies,
    breaks: (next) => {
      let current = chosen
      for (const target of next.slice(chosen.length)) {
        if (!fits(target, current)) return 'Together they pass the most total mana value'
        current = [...current, target]
      }
      return undefined
    },
  })
  const text = totalText === '' ? sentence : `${sentence} ${totalText}`

  return (
    <PromptLayout
      text={text}
      // The targets are on the table or in the centre overlay: a sheet over the hand keeps out of the way.
      folded
      choices={
        <>
          {tileCounts.size === 0 ? null : (
            <style data-slot="tile-target-counts">
              {[...tileCounts]
                .map(
                  ([seat, count]) =>
                    `[data-seat-tile='${seat}']::after{content:'${count} ${count === 1 ? 'target' : 'targets'} on this board'}`,
                )
                .join('')}
            </style>
          )}
          <CardChoices
            quick={<QuickSelect buttons={quick} onSet={setChosen} />}
            cards={offTable}
            chosen={chosenIds}
            closed={closed}
            labels={cardLabels}
            objects={byId}
            onPress={onCard}
            label="Legal targets"
            rest={quick.length > 0 ? QUICK_ROW_REM : 0}
            lead={
              players.length === 0 ? undefined : (
                // The players stand on a line of their own over the cards, so a player never reads as a card.
                <div
                  role="group"
                  aria-label="Players"
                  data-slot="player-targets"
                  className="flex flex-wrap items-center gap-2"
                >
                  <span aria-hidden="true" className="text-xs text-muted-foreground">
                    Players
                  </span>
                  {players.map((target) => {
                    const picked = has(target)
                    return (
                      <Button
                        key={targetKey(target)}
                        size="sm"
                        variant="outline"
                        aria-pressed={picked}
                        disabled={(closed || !fits(target, chosen)) && !picked}
                        onClick={() => choose(target)}
                        className={picked ? CHOSEN_BUTTON : undefined}
                      >
                        {picked ? <Check aria-hidden="true" strokeWidth={3} /> : null}
                        {'Player' in target ? <PlayerFace seat={target.Player} /> : null}
                        {labelOf(target)}
                      </Button>
                    )
                  })}
                </div>
              )
            }
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
        </>
      }
    >
      {onPress ? (
        // One target answers on its press; "up to one" may still choose none.
        min === 0 ? (
          <Button
            variant="outline"
            className="h-10 px-4"
            data-slot="no-target"
            onClick={() => answer({ ChooseTargets: { Targets: [] } })}
          >
            No target
          </Button>
        ) : null
      ) : (
        <>
          <Button
            {...anchor('targets.confirm')}
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
        </>
      )}
      {options.can_cancel ? <UndoButton zone={source?.zone} onUndo={() => void answer('Undo')} /> : null}
    </PromptLayout>
  )
}

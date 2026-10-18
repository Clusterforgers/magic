import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ObjectId } from '@/protocol/generated'
import { useSeatNamer } from '../../names'
import { ManaChoice } from '../ManaChoice'
import { markCards, useCardMarks } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { PassControls } from '../PassControls'
import { PromptLayout } from '../PromptLayout'
import { usePrompt } from '../PromptContext'
import { manaByPermanent, priorityText, topOfStack } from '../priorityText'
import type { PromptComponent } from '../types'

const focusCard = (id: ObjectId): void =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()

/**
 * Priority: lands you may play, spells you may cast and mana abilities you may use are legal, the rest of your hand
 * and battlefield is dimmed. A click plays, casts or taps; a permanent with several mana abilities asks which first.
 */
export const PriorityPrompt: PromptComponent<'Priority'> = ({ options }) => {
  const { decision, answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const top = usePlay(useShallow(topOfStack))
  const name = useSeatNamer()
  const mana = useMemo(() => manaByPermanent(options.mana_abilities), [options.mana_abilities])
  const [choosing] = selection.ids
  const choices = choosing === undefined ? undefined : mana.get(choosing)

  const marks = useMemo(
    () =>
      markCards(
        own,
        [...options.playable_lands, ...options.castable, ...mana.keys()],
        choosing === undefined ? [] : [choosing],
      ),
    [own, options.playable_lands, options.castable, mana, choosing],
  )

  const act = (id: ObjectId): void => {
    if (options.playable_lands.includes(id)) {
      answer({ Priority: { PlayLand: id } })
      return
    }
    if (options.castable.includes(id)) {
      answer({ Priority: { CastSpell: id } })
      return
    }
    const abilities = mana.get(id)
    const only = abilities?.at(0)
    if (abilities === undefined || only === undefined) return
    if (abilities.length === 1) answer({ Priority: { ActivateMana: only.source } })
    else if (selection.has(id)) selection.clear()
    else selection.set([id])
  }
  useCardMarks(marks, act)

  const pick = (option: ManaOption): void => {
    answer({ Priority: { ActivateMana: option.source } })
  }
  const cancel = (): void => {
    selection.clear()
    if (choosing !== undefined) focusCard(choosing)
  }

  return (
    <PromptLayout text={priorityText(options, top, decision.player, name)}>
      {choosing !== undefined && choices !== undefined && choices.length > 1 ? (
        <ManaChoice card={choosing} options={choices} onPick={pick} onCancel={cancel} />
      ) : null}
      <PassControls canPass={options.can_pass} />
    </PromptLayout>
  )
}

import { useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ManaOption, ObjectId } from '@/protocol/generated'
import { useSeatNamer } from '../../names'
import type { MenuTarget } from '../../manual/menuStore'
import { ManaChoice } from '../ManaChoice'
import { handNotes } from '../../handReason'
import { AbilityChooser } from '../AbilityChooser'
import { abilityPlays, activationNotes, mergeNotes, playsOf } from '../activations'
import type { Play } from '../activations'
import { markCards, NO_NOTES, useCardMarks, useCardNotes } from '../marks'
import type { CardNotes } from '../marks'
import { useOwnCardIds } from '../ownCards'
import { PassControls } from '../PassControls'
import { PromptLayout } from '../PromptLayout'
import { usePrompt } from '../PromptContext'
import { manaByPermanent, priorityText, topOfStack } from '../priorityText'
import type { PromptComponent } from '../types'

const focusCard = (id: ObjectId): void =>
  document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)?.focus()

/** The card button a click came from, else the first button the table draws for the card. */
const elementOf = (id: ObjectId, from: HTMLElement | undefined): HTMLElement | null =>
  from ?? document.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)

/**
 * Priority: lands you may play, spells you may cast, mana abilities you may use and abilities you may activate (on a
 * permanent, or on a card of your graveyard, exile or command zone) are legal, the rest of your hand and battlefield
 * is dimmed. A click plays, casts, taps or activates; a card with an ability and another play asks which first, and a
 * permanent with several mana abilities and nothing else asks which mana.
 */
export const PriorityPrompt: PromptComponent<'Priority'> = ({ options }) => {
  const { decision, answer, selection } = usePrompt()
  const own = useOwnCardIds()
  const top = usePlay(useShallow(topOfStack))
  const name = useSeatNamer()
  const seat = usePlay((state) => state.seat)
  const view = usePlay((state) => state.view)
  const objectsById = usePlay((state) => state.objectsById)
  const mana = useMemo(() => manaByPermanent(options.mana_abilities), [options.mana_abilities])
  const abilities = useMemo(() => abilityPlays(options, objectsById, seat), [options, objectsById, seat])
  const [choosing] = selection.ids
  const choices = choosing === undefined ? undefined : mana.get(choosing)
  const [asking, setAsking] = useState<MenuTarget | null>(null)
  const playsFor = (id: ObjectId): readonly Play[] =>
    playsOf(id, objectsById[id]?.name ?? 'the card', options, abilities, mana.get(id) ?? [])

  const marks = useMemo(
    () =>
      markCards(
        own,
        [...options.playable_lands, ...options.castable, ...mana.keys(), ...abilities.keys()],
        choosing === undefined ? [] : [choosing],
      ),
    [own, options.playable_lands, options.castable, mana, abilities, choosing],
  )

  const act = (id: ObjectId, from?: HTMLElement): void => {
    if (abilities.has(id)) {
      const all = playsFor(id)
      const [only] = all
      if (all.length === 1 && only !== undefined) answer({ Priority: only.action })
      else if (asking?.id === id) setAsking(null)
      else {
        const element = elementOf(id, from)
        if (element === null) return
        const { left, bottom } = element.getBoundingClientRect()
        setAsking({ id, x: left, y: bottom, opener: element })
      }
      return
    }
    if (options.playable_lands.includes(id)) {
      answer({ Priority: { PlayLand: id } })
      return
    }
    if (options.castable.includes(id)) {
      answer({ Priority: { CastSpell: id } })
      return
    }
    const manaAbilities = mana.get(id)
    const only = manaAbilities?.at(0)
    if (manaAbilities === undefined || only === undefined) return
    if (manaAbilities.length === 1) answer({ Priority: { ActivateMana: only.source } })
    else if (selection.has(id)) selection.clear()
    else selection.set([id])
  }
  useCardMarks(marks, act)
  const notes = useMemo<CardNotes>(() => {
    const hand = view === null ? NO_NOTES : handNotes(view, objectsById, options)
    return mergeNotes(hand, activationNotes(abilities))
  }, [view, objectsById, options, abilities])
  useCardNotes(notes)

  const pick = (option: ManaOption): void => {
    answer({ Priority: { ActivateMana: option.source } })
  }
  const cancel = (): void => {
    selection.clear()
    if (choosing !== undefined) focusCard(choosing)
  }
  const choose = (play: Play): void => {
    setAsking(null)
    answer({ Priority: play.action })
  }

  return (
    <PromptLayout text={priorityText(options, top, decision.player, name)}>
      {choosing !== undefined && choices !== undefined && choices.length > 1 ? (
        <ManaChoice card={choosing} options={choices} onPick={pick} onCancel={cancel} />
      ) : null}
      {asking === null ? null : (
        <AbilityChooser
          target={asking}
          plays={playsFor(asking.id)}
          onPick={choose}
          onClose={() => setAsking(null)}
        />
      )}
      <PassControls canPass={options.can_pass} />
    </PromptLayout>
  )
}

/** The questions of effects in plain words: who asks, what is asked, which keys answer, and what each answer is called. */
import type { ChoiceOption, ModeOption, PlayerId } from '@/protocol/generated'
import { countOf } from './combatText'

/** The keys that choose the first ten entries of a list, in order: 1 to 9, then 0. */
export const NUMBER_KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'] as const

/** The key that chooses the entry at `index` (from 0), if it has one. */
export function numberKey(index: number): string | undefined {
  return NUMBER_KEYS[index]
}

/** Who asks, as a sentence names it: the source by its name, or "A card" while the view does not hold it. */
export function askerWords(name: string | undefined): string {
  return name ?? 'A card'
}

/** What a screen reader is told after a yes-or-no question. */
export const YES_NO_KEYS = 'Press Y for yes or N for no.'

/** The yes-or-no question: the engine's words after the asker, else a plain one. */
export function yesNoSentence(asker: string, text: string): string {
  return `${asker}: ${text === '' ? 'do you want to do this?' : text}`
}

/** The option question: the engine's words after the asker, else a plain one. */
export function optionSentence(asker: string, text: string): string {
  return `${asker}: ${text === '' ? 'choose one.' : text}`
}

/** The keys that choose from a list of `count` entries, for a screen reader: "Press 1 to 3 to choose." */
export function numberKeysWords(count: number): string {
  if (count <= 1) return 'Press 1 to choose it.'
  if (count === 2) return 'Press 1 or 2 to choose.'
  if (count <= 9) return `Press 1 to ${count} to choose.`
  if (count === 10) return 'Press 1 to 9, or 0, to choose.'
  return 'Press 1 to 9, or 0, to choose one of the first ten.'
}

/** The keys of the mode prompt for a screen reader: they answer when one mode is asked for, else they choose or take back. */
export function modeKeysWords(count: number, single: boolean): string {
  if (single) return numberKeysWords(count)
  const range = count <= 1 ? '1' : count <= 9 ? `1 to ${count}` : '1 to 9, or 0,'
  return `Press ${range} to choose a mode or take it back.`
}

/** One option as its button names it; the viewer's own seat says so. */
export function optionLabel(
  option: ChoiceOption,
  seatName: (seat: PlayerId) => string,
  viewer: PlayerId | null,
): string {
  if ('Player' in option) {
    const name = seatName(option.Player)
    return option.Player === viewer ? `${name} (you)` : name
  }
  return 'Color' in option ? option.Color : option.Label
}

/** One mode as its button names it: the card's name for it, else its place on the card. */
export function modeLabel(mode: ModeOption): string {
  return mode.label ?? `Mode ${mode.index + 1}`
}

const WORDS = ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten']

const numberWord = (n: number): string => WORDS[n] ?? String(n)

/** How many modes are asked for: "one mode", "two modes", "up to two modes", "1 to 3 modes". */
export function modeBounds(min: number, max: number): string {
  const modes = (n: number): string => `${numberWord(n)} ${n === 1 ? 'mode' : 'modes'}`
  if (min === max) return modes(max)
  if (min === 0) return `up to ${modes(max)}`
  return `${min} to ${modes(max)}`
}

/** What a prompt for modes knows when it speaks about the modes chosen so far. */
export interface ModesAsked {
  asker: string
  min: number
  max: number
  repeat: boolean
  /** How many modes are chosen so far, a repeated one counted each time. */
  chosen: number
}

/** The live sentence of the mode prompt: the question at first, then what is chosen and what is left to do. */
export function modesSentence({ asker, min, max, repeat, chosen }: ModesAsked): string {
  if (chosen === 0) {
    const more = repeat && max > 1 ? ', a mode more than once if you like' : ''
    return `${asker}: choose ${modeBounds(min, max)}${more}${min === 0 ? ', or none' : ''}.`
  }
  if (chosen < min) {
    return `${asker}: ${chosen} of ${countOf(min, 'mode')} chosen. Choose ${min - chosen} more.`
  }
  const head = `${asker}: ${countOf(chosen, 'mode')} chosen.`
  if (chosen >= max) return `${head} Confirm the modes, or take one back.`
  return `${head} Confirm the modes, or choose up to ${max - chosen} more.`
}

/** The modes after a press on `index`: a chosen mode is taken back, unless modes may repeat, when it is chosen again. */
export function pressedMode(
  chosen: readonly number[],
  index: number,
  { max, repeat }: { max: number; repeat: boolean },
): number[] {
  if (repeat) return chosen.length < max ? [...chosen, index] : [...chosen]
  if (chosen.includes(index)) return chosen.filter((mode) => mode !== index)
  return chosen.length < max ? [...chosen, index] : [...chosen]
}

/** The answer for the modes chosen: their indices in the order they happen, which is the order on the card. */
export function modesAnswer(chosen: readonly number[]): number[] {
  return [...chosen].sort((a, b) => a - b)
}

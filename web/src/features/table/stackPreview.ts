/** What the preview of a stack entry says it resolves: the spell or ability, its source, its text and its targets. */
import type { ObjectView } from '@/protocol/generated'
import { targetName } from './eventText'
import type { SeatNamer } from './names'
import type { StackEntry } from './stackTargets'
import type { TargetNamer } from './stackText'

/** The preview's stack block as words. */
export interface StackSummary {
  /** "Fireball, cast by Bo", "Triggered ability of Soul Warden". */
  heading: string
  /** The rules the entry resolves, one line each; an ability always has one, a spell's card text is the card's own. */
  rules: readonly string[]
  /** One line a target, "Hill Giant" or "Bo (no longer a legal target)"; empty while it aims at nothing. */
  targets: readonly string[]
}

/** The text said when nothing of an ability's words reaches the viewer. */
export const TEXT_NOT_SHOWN = 'The text of this ability is not shown.'

const TRIGGER_WORDS = /^(when|whenever|at)\b/i

/** Whether the object is on the stack. */
export const isStacked = (object: ObjectView): boolean => object.zone === 'Stack'

/** The object whose card the preview draws large: an ability's source while the view holds it, else the object. */
export function shownObject(object: ObjectView, source: ObjectView | undefined): ObjectView {
  return object.kind === 'Ability' && source !== undefined ? source : object
}

/** Targets one line each; a target that is gone is struck from the legal ones in words. */
function targetLines(entry: StackEntry | undefined, names: TargetNamer): string[] {
  return (entry?.targets ?? []).map(({ target, gone }) =>
    gone ? `${targetName(target, names)} (no longer a legal target)` : targetName(target, names),
  )
}

/**
 * The words that say what a stack entry resolves. `source` is the ability's source object when the view holds it;
 * `entry` carries the targets. Reads only what the viewer's view holds: a face-down spell or an ability of a source
 * the viewer may not look at names no card.
 */
export function stackSummary(
  object: ObjectView,
  source: ObjectView | undefined,
  entry: StackEntry | undefined,
  seat: SeatNamer,
  names: TargetNamer,
): StackSummary {
  const targets = targetLines(entry, names)
  if (object.kind === 'Ability') {
    const from = object.name === '' ? 'a face-down permanent' : object.name
    const text = object.rules_text
    const kind = text !== undefined && TRIGGER_WORDS.test(text) ? 'Triggered' : 'Activated'
    const heading = text === undefined ? `Ability of ${from}` : `${kind} ability of ${from}`
    const byHand = (source?.manual ?? []).map((clause) => clause.text)
    const rules = text !== undefined ? [text] : byHand.length > 0 ? byHand : [TEXT_NOT_SHOWN]
    return {
      heading: `${heading}, controlled by ${seat(object.controller)}`,
      rules,
      targets,
    }
  }
  const by = seat(object.controller)
  if (object.face_down === true && object.name === '') {
    return {
      heading: `Face-down spell, cast by ${by}`,
      rules: ['Its card is face down: only the player who cast it may look at it.'],
      targets,
    }
  }
  const copy = object.copy !== undefined
  const what = copy ? `Copy of ${object.name}` : object.name
  return {
    heading: `${what}, ${copy ? 'controlled' : 'cast'} by ${by}`,
    rules: [],
    targets,
  }
}

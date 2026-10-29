/** What the save bar says about saving the draft and whether anything stands in the way. Pure, so tables test it. */
import { nameProblem } from '@/features/decks/rules'

/** The state of saving: nothing to save, ready, in flight, or held back by a problem the person can fix. */
export type SaveKind = 'saved' | 'ready' | 'saving' | 'blocked'

/** What the bar shows. */
export interface SaveStatus {
  kind: SaveKind
  /** One sentence for the bar. */
  text: string
  /** The reason Save does nothing now, or null when pressing it saves (or when there is nothing to save). */
  blocker: string | null
}

/** What the status is made from. */
export interface SaveInput {
  /** The draft differs from what is saved. */
  dirty: boolean
  saving: boolean
  /** The draft's name. */
  name: string
  /** Legality issues the live check found; they do not stop a save but are said. */
  issues: number
  /** The format the issues are against, as people say it. */
  format: string
}

/**
 * Nothing blocks saving except an unusable name and a save already on its way; legality issues never do (a deck that is
 * not legal is still saved, with its issues listed), so the bar names them and says it can still be saved.
 */
export function saveStatus({ dirty, saving, name, issues, format }: SaveInput): SaveStatus {
  if (saving) return { kind: 'saving', text: 'Saving…', blocker: 'A save is on its way.' }
  const problem = nameProblem(name)
  if (dirty && problem !== null) return { kind: 'blocked', text: `Cannot save yet. ${problem}`, blocker: problem }
  if (!dirty) return { kind: 'saved', text: 'All changes saved.', blocker: null }
  const legality =
    issues === 0
      ? ''
      : ` The deck has ${issues} ${issues === 1 ? 'issue' : 'issues'} in ${format}; it can still be saved.`
  return { kind: 'ready', text: `Unsaved changes.${legality}`, blocker: null }
}

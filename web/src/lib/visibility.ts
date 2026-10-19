/** The words for who may find a game, shared by the create page and the lobby so the promise and the status agree. */
import type { Visibility } from '@/protocol/generated'

/** How one visibility reads: its name as a choice, what it means for the game, and the status line of the lobby. */
export interface VisibilityWords {
  /** The choice on the create page. */
  choice: string
  /** What choosing it does, shown beside the choice. */
  consequence: string
  /** What the lobby says about a game that has it. */
  status: string
}

/** The two visibilities, with the words each one is shown under. */
export const VISIBILITY_WORDS: Readonly<Record<Visibility, VisibilityWords>> = {
  public: {
    choice: 'Listed',
    consequence: 'Listed in the game browser',
    status: 'Listed in the game browser',
  },
  invite: {
    choice: 'Invite only',
    consequence: 'Only people with your link or code',
    status: 'Invite only',
  },
}

/** The choices in the order the create page offers them; the first is the default. */
export const VISIBILITY_CHOICES: readonly Visibility[] = ['public', 'invite']

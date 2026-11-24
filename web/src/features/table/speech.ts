/**
 * The viewer's own seat in second person: a namer marks the viewer's seat (`VIEWER`) and `speak` turns a finished
 * sentence into one addressed to the viewer, so "Bo plays Plains" stays and the viewer reads "You play Plains" and
 * "Your turn", never "You plays" or "You's turn".
 */
import type { PlayerId } from '@/protocol/generated'
import type { SeatNamer } from './names'

// Word joiners around the name: invisible, and a sentence that skips `speak` still reads "You".
/** The name a voiced namer gives the viewer's seat; `speak` resolves it. */
export const VIEWER = '⁠You⁠'

/** `name` with the viewer's own seat marked as `VIEWER`; `name` itself while there is no viewer (a spectator). */
export function voiced(name: SeatNamer, viewer: PlayerId | null | undefined): SeatNamer {
  if (viewer === null || viewer === undefined) return name
  return (seat) => (seat === viewer ? VIEWER : name(seat))
}

const IRREGULAR: Record<string, string> = {
  is: 'are',
  has: 'have',
  was: 'were',
  does: 'do',
  goes: 'go',
  "isn't": "aren't",
  "hasn't": "haven't",
  "wasn't": "weren't",
  "doesn't": "don't",
}

const ADVERBS = new Set(['also', 'then', 'only', 'now', 'still', 'again', 'never', 'always', 'no', 'longer'])

/** A third-person verb in its second-person form ("plays" to "play", "scries" to "scry"), or null for any other word. */
export function secondPerson(word: string): string | null {
  const lower = word.toLowerCase()
  const irregular = IRREGULAR[lower]
  if (irregular !== undefined) return irregular
  if (!/^[a-z]+s$/.test(lower) || lower.endsWith('ss') || lower.endsWith('us')) return null
  if (/(ss|sh|ch|x|z)es$/.test(lower)) return lower.slice(0, -2)
  if (lower.length > 4 && /[^aeiou]ies$/.test(lower)) return `${lower.slice(0, -3)}y`
  return lower.slice(0, -1)
}

// The viewer is the subject where a clause starts: at the start of the text or after a mark that ends a clause.
const opensClause = (before: string): boolean => before.trim() === '' || /[.!?:;,]\s*$/.test(before)
const opensSentence = (before: string): boolean => before.trim() === '' || /[.!?]\s*$/.test(before)

// The rest of a clause whose subject is the viewer: its pronouns are the viewer's.
function ownPronouns(rest: string): string {
  const end = rest.search(/[.;]|⁠/)
  const clause = end === -1 ? rest : rest.slice(0, end)
  const after = end === -1 ? '' : rest.slice(end)
  const own = clause
    .replace(/\btheir\b/g, 'your')
    .replace(/\bthemselves\b/g, 'yourself')
    .replace(/\bthey\b/g, 'you')
  return own + after
}

// After the subject: the verb (past any adverbs) in its second-person form, or null when no such verb follows.
function conjugate(rest: string): string | null {
  const words = /^(\s+)((?:(?:also|then|only|now|still|again|never|always|no|longer)\s+)*)([A-Za-z']+)/.exec(
    rest,
  )
  if (words === null) return null
  const [whole, space = '', adverbs = '', verb = ''] = words
  if (ADVERBS.has(verb.toLowerCase())) return null
  const spoken = secondPerson(verb)
  if (spoken === null) return null
  return `${space}${adverbs}${spoken}${rest.slice(whole.length)}`
}

/** `text` with every `VIEWER` turned into "you" or "your", and the verb and pronouns of a clause the viewer starts made to agree. */
export function speak(text: string): string {
  if (!text.includes(VIEWER)) return text
  let out = ''
  let rest = text
  for (let at = rest.indexOf(VIEWER); at !== -1; at = rest.indexOf(VIEWER)) {
    const before = out + rest.slice(0, at)
    let after = rest.slice(at + VIEWER.length)
    const capital = opensSentence(before)
    if (after.startsWith("'s") || after.startsWith('’s')) {
      out = `${before}${capital ? 'Your' : 'your'}`
      rest = after.slice(2)
      continue
    }
    const agreed = opensClause(before) ? conjugate(after) : null
    if (agreed !== null) after = ownPronouns(agreed)
    out = `${before}${capital ? 'You' : 'you'}`
    rest = after
  }
  return out + rest
}

/**
 * The Delves rules in short: how a run goes and how Spirits work (docs/delves.md), as folding entries on the Delves
 * page, and the note on bringing a fallen hero back that the run page shows beside a fight.
 */
import { GhostIcon } from 'lucide-react'

/** How a Spirit works, one line each. */
export const SPIRIT_RULES: readonly string[] = [
  'A hero at 0 life becomes a Restless Spirit while a teammate lives; the party loses only when every hero has fallen.',
  "Enemies can't attack a Spirit or target it.",
  'A Spirit starts with 1 ectoplasm and gains 1 each of its turns, and spends it on haunt actions: Wail, Ward, Whisper and Hinder cost 1, Howl costs 3.',
  'With 5 ectoplasm a Spirit can Rise Again in its main phase: back in the fight with 10 life and five fresh cards.',
]

/** The ways a fallen hero comes back, one line each. */
export const REVIVAL_WAYS: readonly string[] = [
  'Rise Again: a Spirit with 5 ectoplasm returns with 10 life and five cards.',
  'Phoenix Feather: a relic whose holder pays 6 life, once each fight, to return every fallen teammate with 8 life.',
  'Win the fight: every Spirit comes back at 10 life for the next one, and a Campfire rest heals 20.',
]

const ENTRIES: readonly { title: string; lines: readonly string[] }[] = [
  {
    title: 'How a delve goes',
    lines: [
      'Your party fights a ladder of bosses; between two bosses you vote for a camp: Treasure, Shop, Event or Campfire.',
      'Life carries over between fights. A lost fight costs one of the party’s 3 hearts and is retried.',
    ],
  },
  { title: 'How Spirits work', lines: SPIRIT_RULES },
  { title: 'Bringing a hero back', lines: REVIVAL_WAYS },
]

export function DelvesRules() {
  return (
    <section aria-labelledby="delves-rules" className="surface grid gap-2 p-4">
      <h2 id="delves-rules" className="text-lg font-semibold">
        Rules in short
      </h2>
      {ENTRIES.map((entry) => (
        <details key={entry.title} data-slot="delves-rule" className="group rounded-lg border p-2">
          <summary className="cursor-pointer font-medium">{entry.title}</summary>
          <ul className="mt-2 grid list-disc gap-1 ps-5 text-sm">
            {entry.lines.map((text) => (
              <li key={text}>{text}</li>
            ))}
          </ul>
        </details>
      ))}
    </section>
  )
}

/** The run page's note on fallen heroes: what a Spirit does and the ways back. */
export function RevivalNote() {
  return (
    <aside
      aria-label="Fallen heroes"
      data-slot="revival-note"
      className="grid gap-1 rounded-lg border border-dashed border-spirit p-3 text-sm"
    >
      <p className="inline-flex items-center gap-2 font-medium text-spirit">
        <GhostIcon aria-hidden="true" className="size-4" />
        Fall in a fight, and you haunt it as a Restless Spirit.
      </p>
      <ul className="grid list-disc gap-1 ps-5">
        {REVIVAL_WAYS.map((text) => (
          <li key={text}>{text}</li>
        ))}
      </ul>
    </aside>
  )
}

/** One glossary line: the names that point at it, joined by `|` (the first is the title), and its reminder text. */
export type Entry = readonly [names: string, text: string]

/** What the server accepts of a deck name and a pasted list, checked before anything is sent. */

/** Longest deck name, in characters (`MAX_DECK_NAME_CHARS` in `crates/mtg-protocol/src/rest/decks.rs`). */
export const MAX_DECK_NAME_CHARS = 100
/** Longest pasted list, in characters (`MAX_IMPORT_CHARS`). */
export const MAX_IMPORT_CHARS = 100_000
/** Most lines in a pasted list (`MAX_IMPORT_LINES`). */
export const MAX_IMPORT_LINES = 1_000
/** The server's import body limit in bytes: a file above it holds more than a list may. */
export const MAX_IMPORT_FILE_BYTES = 640 * 1024

// Control characters and the invisible or direction-changing marks `is_unsafe_in_names` refuses in `api.rs`.
const UNSAFE_IN_NAMES = /[\p{Cc}​-‏‪-‮⁠-⁤⁦-⁩﻿]/u

const group = (count: number): string => count.toLocaleString('en-US')

/** What is wrong with a deck name, in words, or null when the server will take it. */
export function nameProblem(name: string): string | null {
  if (name.trim() === '') return 'Enter a name for the deck.'
  const length = [...name].length
  if (length > MAX_DECK_NAME_CHARS) {
    return `The name is at most ${MAX_DECK_NAME_CHARS} characters (this one has ${length}).`
  }
  if (UNSAFE_IN_NAMES.test(name)) return 'The name cannot contain control characters or invisible marks.'
  return null
}

/** What is wrong with a pasted list, in words, or null when the server will read it. */
export function listProblem(text: string): string | null {
  if (text.trim() === '') return 'Paste a deck list or choose a file.'
  // UTF-16 units are never fewer than characters, so a short string needs no exact count.
  const chars = text.length > MAX_IMPORT_CHARS ? [...text].length : text.length
  if (chars > MAX_IMPORT_CHARS) {
    return `The list is at most ${group(MAX_IMPORT_CHARS)} characters (this one has ${group(chars)}).`
  }
  const lines = text.split('\n').length - (text.endsWith('\n') ? 1 : 0)
  if (lines > MAX_IMPORT_LINES) {
    return `The list is at most ${group(MAX_IMPORT_LINES)} lines (this one has ${group(lines)}).`
  }
  return null
}

/** What is wrong with a chosen file, or null when it can be read as a list. */
export function fileProblem(file: { size: number }): string | null {
  return file.size > MAX_IMPORT_FILE_BYTES
    ? `That file is too big for a deck list, which is at most ${group(MAX_IMPORT_CHARS)} characters.`
    : null
}

/** A deck name suggested by a file name: the name without its extension, or an empty string. */
export function nameFromFile(fileName: string): string {
  return fileName
    .replace(/\.[^./\\]{1,8}$/, '')
    .trim()
    .slice(0, MAX_DECK_NAME_CHARS)
}

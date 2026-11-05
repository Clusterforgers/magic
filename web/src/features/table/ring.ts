/** The words of a seat's Ring (CR 701.54). Pure. */

/** "Bo's Ring: level 2, bearer Elf" (or "no bearer"). */
export function ringWords(owner: string, level: number, bearer: string | undefined): string {
  return `${owner}'s Ring: level ${level}, ${bearer === undefined ? 'no Ring-bearer' : `Ring-bearer ${bearer}`}`
}

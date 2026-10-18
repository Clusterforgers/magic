/** Reading a stylesheet as text, for tests that guard what it must and must not contain. */

/** The text between the braces that open after `head`, nested braces included. */
export function blockAfter(source: string, head: string): string {
  const start = source.indexOf(head)
  if (start < 0) throw new Error(`the stylesheet has no "${head}"`)
  const open = source.indexOf('{', start)
  let depth = 0
  for (let at = open; at < source.length; at += 1) {
    if (source[at] === '{') depth += 1
    if (source[at] === '}') depth -= 1
    if (depth === 0) return source.slice(open + 1, at)
  }
  throw new Error(`"${head}" never closes`)
}

/** The rules of a block that holds no nested blocks of its own, as selector and body; `@keyframes` are left out. */
export function rulesIn(block: string): Array<{ selector: string; body: string }> {
  const flat = block.replace(/@keyframes[^{]*\{(?:[^{}]*\{[^{}]*\})*[^{}]*\}/g, '')
  return [...flat.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((match) => ({
    selector: (match[1] ?? '').replace(/\/\*[\s\S]*?\*\//g, '').trim(),
    body: match[2] ?? '',
  }))
}

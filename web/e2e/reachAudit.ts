// The measurements of the reach gate: every control a page draws can be seen and pressed at any window size.
// `reach.spec.ts` runs them in Playwright. It imports nothing, so Node can load this file as it stands.

/** A window size in CSS px and the device pixels per CSS px (a browser zoom of 150 % is a smaller window at 1.5). */
export interface Screen {
  name: string
  width: number
  height: number
  scale?: number
  /** A phone, either way up: the table has a layout of its own there. */
  phone?: true
  /** One of the few screens every stop of the gate is held to; the stops that crowd most are held to all of them. */
  core?: true
}

/** The screens of the reach gate: laptops and desktops first (short, zoomed, high-DPI, under browser bars, ultrawide), then a tablet and a phone each way up. */
export const SCREENS: readonly Screen[] = [
  { name: 'short laptop 1280x600', width: 1280, height: 600, core: true },
  { name: 'laptop 1280x720', width: 1280, height: 720 },
  { name: 'laptop 1280x720 under browser bars', width: 1280, height: 580 },
  { name: 'laptop 1366x768', width: 1366, height: 768, core: true },
  { name: 'laptop 1366x768 under browser bars', width: 1366, height: 630, core: true },
  { name: 'laptop 1440x900', width: 1440, height: 900 },
  { name: 'laptop 1536x864', width: 1536, height: 864 },
  { name: 'laptop 1536x864 under browser bars', width: 1536, height: 730 },
  { name: 'laptop 1600x900', width: 1600, height: 900 },
  { name: '16:10 laptop 1280x800', width: 1280, height: 800 },
  { name: '3:2 laptop 1500x1000', width: 1500, height: 1000 },
  { name: 'desktop 1920x1080', width: 1920, height: 1080, core: true },
  { name: 'desktop 1920x1080 under browser bars', width: 1920, height: 940 },
  { name: '1920x1080 at 125 %', width: 1536, height: 864, scale: 1.25, core: true },
  { name: '1920x1080 at 150 %', width: 1280, height: 720, scale: 1.5 },
  { name: '1920x1080 at 175 %', width: 1097, height: 617, scale: 1.75, core: true },
  { name: '1920x1080 at 175 % under browser bars', width: 1097, height: 540, scale: 1.75 },
  { name: '1920x1080 at 200 %', width: 960, height: 540, scale: 2, core: true },
  { name: '1366x768 at 125 %', width: 1093, height: 614, scale: 1.25 },
  { name: 'high-DPI laptop 1440x900 at 2', width: 1440, height: 900, scale: 2 },
  { name: 'ultrawide 2560x1080', width: 2560, height: 1080, core: true },
  { name: 'ultrawide 3440x1440', width: 3440, height: 1440 },
  { name: 'wide low window 1600x500', width: 1600, height: 500 },
  { name: 'tablet on its side 1024x768', width: 1024, height: 768, core: true },
  { name: 'tablet 768x1024', width: 768, height: 1024 },
  { name: 'phone 390x844', width: 390, height: 844, core: true, phone: true },
  { name: 'phone on its side 844x390', width: 844, height: 390, phone: true },
]

/** What `reach` is asked. */
export interface ReachAsk {
  /** Regions whose controls are cut off or lie under each other on purpose (a fan of cards, a strip that scrolls). */
  exempt: string[]
  /** The page is meant to scroll down, so a control below the fold is reached by scrolling. */
  scrolls: boolean
  /** Layers that lie over the page on purpose while they are up (a choice overlay): what is under one is not judged covered. */
  overlays?: string[]
}

/** What `reach` found in a page; plain data, so it crosses from the page to the test. */
export interface Reach {
  /** The window as the page is laid out in it. */
  inner: { width: number; height: number }
  /** How big the document is. */
  scroll: { width: number; height: number }
  /** How many controls were judged. */
  controls: number
  /** One line for every control that cannot be seen whole or pressed. */
  findings: string[]
}

/**
 * Runs in the page: judges every drawn control (button, link, field, tab, option and whatever takes the keyboard).
 * A control is reached when at least `SEEN` of it is inside the window and every box that clips it, a box that scrolls
 * counting as one that can show it, and when a press on it lands on it. It cannot use anything outside its own body,
 * because Playwright sends it as source.
 */
export function reach(ask: ReachAsk): Reach {
  const SEEN = 0.9
  const root = document.documentElement
  const inner = { width: root.clientWidth, height: root.clientHeight }
  const scroll = { width: root.scrollWidth, height: root.scrollHeight }
  const styles = new Map<Element, CSSStyleDeclaration>()
  const styleOf = (element: Element): CSSStyleDeclaration => {
    let style = styles.get(element)
    if (style === undefined) {
      style = getComputedStyle(element)
      styles.set(element, style)
    }
    return style
  }
  const named = (element: Element): string => {
    const tag = element.tagName.toLowerCase()
    const slot = element.getAttribute('data-slot')
    const text = (
      element.getAttribute('aria-label') ??
      element.textContent ??
      element.getAttribute('title') ??
      ''
    )
      .replace(/\s+/g, ' ')
      .trim()
      .slice(0, 28)
    return `${tag}${slot === null ? '' : `[${slot}]`}${text === '' ? '' : ` "${text}"`}`
  }
  const SELECTOR = [
    'button',
    'a[href]',
    'input:not([type="hidden"])',
    'select',
    'textarea',
    'summary',
    '[role="button"]',
    '[role="link"]',
    '[role="tab"]',
    '[role="menuitem"]',
    '[role="menuitemradio"]',
    '[role="menuitemcheckbox"]',
    '[role="checkbox"]',
    '[role="radio"]',
    '[role="switch"]',
    '[role="option"]',
    '[role="slider"]',
    '[role="combobox"]',
    '[tabindex]:not([tabindex="-1"])',
  ].join(',')
  /** What is pressed or typed in, as opposed to a box that takes the keyboard for its contents. */
  const PRESSED =
    'button, a[href], input, select, textarea, summary, [role="button"], [role="link"], [role="tab"]'
  /** A box clipped to a pixel: its words are for a screen reader alone. */
  const readerOnly = (element: Element, style: CSSStyleDeclaration): boolean =>
    style.position === 'absolute' &&
    (style.clipPath !== 'none' || (element.clientWidth <= 1 && element.clientHeight <= 1))
  const exempt = ask.exempt.join(',')
  const overlays = (ask.overlays ?? []).join(',')
  const scrolling = new Set(['auto', 'scroll'])
  const clipping = new Set(['hidden', 'clip'])
  /** The box that holds an element in place of its parent: the nearest positioned one for an absolute element. */
  const holder = (element: Element, style: CSSStyleDeclaration): Element | null => {
    if (style.position === 'fixed') return null
    if (style.position !== 'absolute') return element.parentElement
    let up = element.parentElement
    while (up !== null && styleOf(up).position === 'static') up = up.parentElement
    return up
  }
  interface Span {
    start: number
    end: number
  }
  /** Moves a span into a box that scrolls, as scrolling to it would; a span longer than the box fills it. */
  const scrolled = (span: Span, box: Span): Span => {
    const size = Math.min(span.end - span.start, box.end - box.start)
    const start = Math.min(Math.max(span.start, box.start), box.end - size)
    return { start, end: start + size }
  }
  /** Moves a line of text into a box that scrolls and keeps it whole: scrolling shows every part of it in turn. */
  const slid = (span: Span, box: Span): Span => {
    const size = span.end - span.start
    const start = Math.min(Math.max(span.start, box.start), Math.max(box.start, box.end - size))
    return { start, end: start + size }
  }
  const cut = (span: Span, box: Span): Span => ({
    start: Math.max(span.start, box.start),
    end: Math.max(Math.max(span.start, box.start), Math.min(span.end, box.end)),
  })

  const findings: string[] = []
  const cutControls = new Set<Element>()
  let controls = 0
  // Under a dialog that takes the page (a modal) only the dialog's own controls are in reach.
  const modals = Array.from(document.querySelectorAll('[role="dialog"][aria-modal="true"], dialog[open]'))
  const modal = modals[modals.length - 1] ?? null
  for (const element of Array.from(document.body.querySelectorAll(SELECTOR))) {
    if (modal !== null && !modal.contains(element)) continue
    if (element.closest('[inert], [aria-hidden="true"], [hidden]') !== null) continue
    if (exempt !== '' && element.closest(exempt) !== null) continue
    const style = styleOf(element)
    const rect = element.getBoundingClientRect()
    if (style.display === 'none' || style.visibility !== 'visible') continue
    // The contents of a closed disclosure and of a list row far off screen keep boxes but are not drawn (`content-visibility`).
    if (!element.checkVisibility({ contentVisibilityAuto: true })) continue
    // A field drawn by its label (a radio under a card, a file input under a button) has no box of its own to press.
    if (rect.width < 4 || rect.height < 4 || Number(style.opacity) === 0) continue
    if (readerOnly(element, style)) continue
    // A box that only takes the keyboard for what it holds (a tab's panel, a list that scrolls) is judged by its controls.
    if (!element.matches(PRESSED) && element.querySelector(SELECTOR) !== null) continue
    controls += 1
    const full = { x: { start: rect.left, end: rect.right }, y: { start: rect.top, end: rect.bottom } }
    let x: Span = full.x
    let y: Span = full.y
    let clipper: Element | null = null
    let dim = 1
    let own: Element | null = element
    let ownStyle = style
    const scrollers: Element[] = []
    let spoken = false
    // Every box on the way out either scrolls (it can show the control) or clips it for good.
    for (;;) {
      dim *= Number(ownStyle.opacity)
      const up: Element | null = holder(own, ownStyle)
      if (up === null || up === root || up === document.body) break
      const upStyle = styleOf(up)
      if (readerOnly(up, upStyle)) {
        spoken = true
        break
      }
      if (upStyle.display !== 'contents') {
        const box = up.getBoundingClientRect()
        const across = { start: box.left + up.clientLeft, end: box.left + up.clientLeft + up.clientWidth }
        const down = { start: box.top + up.clientTop, end: box.top + up.clientTop + up.clientHeight }
        const before = (x.end - x.start) * (y.end - y.start)
        if (scrolling.has(upStyle.overflowX)) x = scrolled(x, across)
        else if (clipping.has(upStyle.overflowX)) x = cut(x, across)
        if (scrolling.has(upStyle.overflowY)) y = scrolled(y, down)
        else if (clipping.has(upStyle.overflowY)) y = cut(y, down)
        if (scrolling.has(upStyle.overflowX) || scrolling.has(upStyle.overflowY)) scrollers.push(up)
        if ((x.end - x.start) * (y.end - y.start) < before - 0.5 && clipper === null) clipper = up
      }
      own = up
      ownStyle = upStyle
    }
    if (spoken) {
      controls -= 1
      continue
    }
    // The page itself scrolls down where it is meant to, never sideways.
    const windowDown = { start: 0, end: inner.height }
    const beforeWindow = (x.end - x.start) * (y.end - y.start)
    y = ask.scrolls && ownStyle.position !== 'fixed' ? scrolled(y, windowDown) : cut(y, windowDown)
    x = cut(x, { start: 0, end: inner.width })
    const seen = ((x.end - x.start) * (y.end - y.start)) / (rect.width * rect.height)
    const where = `${Math.round(rect.left)},${Math.round(rect.top)} ${Math.round(rect.width)}x${Math.round(rect.height)}`
    if (seen < SEEN) {
      const by =
        clipper !== null && (x.end - x.start) * (y.end - y.start) >= beforeWindow - 0.5
          ? named(clipper)
          : clipper === null
            ? 'the window'
            : `${named(clipper)} and the window`
      findings.push(`cut off: ${named(element)} at ${where}, ${Math.round(seen * 100)} % in view, by ${by}`)
      cutControls.add(element)
      continue
    }
    if (dim < 0.05) {
      findings.push(`not drawn: ${named(element)} at ${where}`)
      continue
    }
    // A press is judged where the control lies now; one that a scroll would bring into view is behind nothing yet.
    const inPlace =
      Math.abs(x.start - Math.max(full.x.start, 0)) < 1 &&
      Math.abs(y.start - Math.max(full.y.start, 0)) < 1 &&
      full.y.end <= inner.height + 0.5 &&
      full.y.start >= -0.5
    if (!inPlace) continue
    const target = element as HTMLElement
    const kept = target.style.pointerEvents
    // A disabled control has its pointer events off, so the hit test would skip it: it is tested as if they were on.
    if (style.pointerEvents === 'none') target.style.pointerEvents = 'auto'
    const spots = [0.5, 0.2, 0.8]
    let hits = 0
    let cover: Element | null = null
    const press = (): void => {
      const at = element.getBoundingClientRect()
      hits = 0
      cover = null
      for (const across of spots) {
        for (const down of spots) {
          const top = document.elementFromPoint(at.left + at.width * across, at.top + at.height * down)
          const label = top?.closest('label')
          const mine =
            top === null ||
            element.contains(top) ||
            top.contains(element) ||
            (label != null && (label.contains(element) || label.getAttribute('for') === element.id))
          if (mine) hits += 1
          else if (cover === null) cover = top
        }
      }
    }
    press()
    // A page or a list that scrolls slides its content under a bar that stays (a sticky footer): the control is
    // pressed where a person would scroll it to, and every scroll is put back.
    if (hits < 5 && (ask.scrolls || scrollers.length > 0)) {
      const was = { x: window.scrollX, y: window.scrollY }
      const held = scrollers.map((box) => ({ box, left: box.scrollLeft, top: box.scrollTop }))
      element.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'instant' })
      press()
      for (const { box, left, top } of held) box.scrollTo({ left, top, behavior: 'instant' })
      window.scrollTo({ left: was.x, top: was.y, behavior: 'instant' })
    }
    target.style.pointerEvents = kept
    const meant = overlays !== '' && (cover as Element | null)?.closest(overlays) != null
    if (hits < 5 && cover !== null && !meant)
      findings.push(`covered: ${named(element)} at ${where}, ${hits} of 9 points free, under ${named(cover)}`)
  }
  // Words: a line of text is read whole or ends in an ellipsis; a box above it that clips it mid-line hides words.
  const cutText = new Set<string>()
  const words = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
  const range = document.createRange()
  for (let node = words.nextNode(); node !== null; node = words.nextNode()) {
    const text = (node.nodeValue ?? '').replace(/\s+/g, ' ').trim()
    const parent = node.parentElement
    if (text === '' || parent === null) continue
    if (modal !== null && !modal.contains(parent)) continue
    if (parent.closest('[inert], [aria-hidden="true"], [hidden], svg, [role="img"]') !== null) continue
    if (exempt !== '' && parent.closest(exempt) !== null) continue
    const style = styleOf(parent)
    if (
      style.display === 'none' ||
      style.visibility !== 'visible' ||
      !parent.checkVisibility({ contentVisibilityAuto: true })
    )
      continue
    if (readerOnly(parent, style)) continue
    // A control that is cut off is one finding; its words are not named again.
    const within = parent.closest(SELECTOR)
    if (within !== null && cutControls.has(within)) continue
    range.selectNodeContents(node)
    for (const line of Array.from(range.getClientRects())) {
      if (line.width < 2 || line.height < 2) continue
      let x: Span = { start: line.left, end: line.right }
      let y: Span = { start: line.top, end: line.bottom }
      let clipper: Element | null = null
      let dim = 1
      let gone = false
      let own: Element | null = parent
      let ownStyle = style
      let first = true
      for (;;) {
        // The text's own box is the first that can clip it; after it, the boxes that hold that one.
        const up: Element | null = first ? own : holder(own, ownStyle)
        const upStyle: CSSStyleDeclaration = first ? ownStyle : up === null ? ownStyle : styleOf(up)
        if (!first) dim *= Number(ownStyle.opacity)
        first = false
        if (up === null || up === root || up === document.body) break
        if (readerOnly(up, upStyle)) {
          gone = true
          break
        }
        if (upStyle.display !== 'contents' && upStyle.display !== 'inline') {
          const box = up.getBoundingClientRect()
          const across = { start: box.left + up.clientLeft, end: box.left + up.clientLeft + up.clientWidth }
          const down = { start: box.top + up.clientTop, end: box.top + up.clientTop + up.clientHeight }
          const before = (x.end - x.start) * (y.end - y.start)
          if (scrolling.has(upStyle.overflowX)) x = slid(x, across)
          else if (clipping.has(upStyle.overflowX)) x = cut(x, across)
          if (scrolling.has(upStyle.overflowY)) y = slid(y, down)
          else if (clipping.has(upStyle.overflowY)) y = cut(y, down)
          const after = (x.end - x.start) * (y.end - y.start)
          if (after < before - 0.5) {
            // A box that ends its text in an ellipsis or after so many lines cuts on purpose and says so.
            const meant =
              upStyle.textOverflow === 'ellipsis' ||
              (upStyle.getPropertyValue('-webkit-line-clamp') || 'none') !== 'none'
            if (meant) {
              // A line past the clamp is not there to read; a line cut across is cut inside the box that means it.
              if (after < 1) gone = true
            } else if (clipper === null) clipper = up
          }
        }
        own = up
        ownStyle = upStyle
        if (gone) break
      }
      if (gone || dim < 0.05) continue
      const beforeWindow = (x.end - x.start) * (y.end - y.start)
      y =
        ask.scrolls && ownStyle.position !== 'fixed'
          ? slid(y, { start: 0, end: inner.height })
          : cut(y, { start: 0, end: inner.height })
      x = cut(x, { start: 0, end: inner.width })
      const left = (x.end - x.start) * (y.end - y.start)
      if (left >= line.width * line.height * SEEN) continue
      const by = clipper !== null ? named(clipper) : 'the window'
      if (clipper === null && left >= beforeWindow - 0.5) continue
      cutText.add(
        `cut text: "${text.slice(0, 32)}" at ${Math.round(line.left)},${Math.round(line.top)}, by ${by}`,
      )
    }
  }
  findings.push(...cutText)
  if (scroll.width > inner.width) findings.push(`scrolls sideways ${scroll.width} > ${inner.width}`)
  if (!ask.scrolls && scroll.height > inner.height)
    findings.push(`scrolls down ${scroll.height} > ${inner.height}`)
  return { inner, scroll, controls, findings }
}

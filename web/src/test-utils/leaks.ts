/** Marks the one kind of element that may carry a seat token: a link meant to be handed to a player. */
export const SHARE_LINK = '[data-share-link]'

/** Everything that could show a secret, share links excepted: markup, text, form values and the tab title. */
export function visibleOutsideShareLinks(root: Element): string {
  const copy = root.cloneNode(true)
  if (!(copy instanceof Element)) throw new Error('the test expected an element')
  copy.querySelectorAll(SHARE_LINK).forEach((link) => link.remove())
  const values = [...copy.querySelectorAll('input, textarea, select')].map((control) =>
    'value' in control && typeof control.value === 'string' ? control.value : '',
  )
  return [copy.innerHTML, copy.textContent ?? '', ...values, document.title].join('\n')
}

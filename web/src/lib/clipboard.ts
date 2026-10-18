/** Puts `text` on the clipboard; false when the browser refuses, so the caller can tell the person to copy by hand. */
export async function copyText(text: string): Promise<boolean> {
  const clipboard: Clipboard | undefined = navigator.clipboard
  if (clipboard !== undefined) {
    try {
      await clipboard.writeText(text)
      return true
    } catch {
      // Denied or unfocused: fall through to the legacy path.
    }
  }
  return legacyCopy(text)
}

// `navigator.clipboard` exists only in secure contexts, and friends may well open a LAN address over plain http.
function legacyCopy(text: string): boolean {
  const area = document.createElement('textarea')
  area.value = text
  area.setAttribute('readonly', '')
  area.style.position = 'fixed'
  area.style.opacity = '0'
  document.body.append(area)
  try {
    area.select()
    return typeof document.execCommand === 'function' && document.execCommand('copy')
  } catch {
    return false
  } finally {
    area.remove()
  }
}

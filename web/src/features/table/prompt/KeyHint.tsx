/** The key that answers, drawn small beside a label. Decoration: the button carries `aria-keyshortcuts` and a label of its own. */
export function KeyHint({ children }: { children: string }) {
  return (
    <kbd
      aria-hidden="true"
      data-slot="key-hint"
      className="ml-2 inline-flex min-w-5 items-center justify-center rounded border border-current/30 px-1 font-mono text-xs opacity-70"
    >
      {children}
    </kbd>
  )
}

/** Strip the WebView's browser affordances so the window behaves like a native
 * desktop app rather than a web page. Reinforces the native WebView2 lockdown
 * done in Rust (`lockdown_webview`). In `tauri dev` we keep reload + devtools for
 * debugging; release builds are fully locked down (here and natively). */
export function installDesktopShell() {
  const isDev = import.meta.env.DEV

  // Right-click menu — suppressed everywhere except editable fields (where a
  // copy/paste menu is genuinely useful). Applies in dev too.
  window.addEventListener('contextmenu', (e) => {
    const target = e.target as HTMLElement | null
    const editable = target?.closest('input, textarea, [contenteditable=""], [contenteditable="true"]')
    if (!editable) e.preventDefault()
  })

  // Dropping a file would otherwise navigate the webview to it.
  window.addEventListener('dragover', (e) => e.preventDefault())
  window.addEventListener('drop', (e) => e.preventDefault())

  // Keep reload + devtools + find/zoom in dev; lock them down in release.
  if (isDev) return

  window.addEventListener('keydown', (e) => {
    const k = e.key.toLowerCase()
    const ctrl = e.ctrlKey || e.metaKey
    const reload = k === 'f5' || (ctrl && k === 'r')
    const devtools = k === 'f12' || (ctrl && e.shiftKey && ['i', 'j', 'c'].includes(k))
    const tools = ctrl && !e.shiftKey && ['f', 'p', 's', 'g', 'u', 'j', 'l'].includes(k)
    const zoom = ctrl && ['+', '=', '-', '_', '0'].includes(k)
    if (reload || devtools || tools || zoom) e.preventDefault()
  })
  window.addEventListener('wheel', (e) => { if (e.ctrlKey) e.preventDefault() }, { passive: false })
}

import { useEffect, useState } from 'react'
import { Copy, Minus, Square, X } from 'lucide-react'
import { StackrIcon } from '../ui/Logo'
import { isTauri } from '../../lib/api'
import { version } from '../../../package.json'

/** Frameless-window title bar: drag region + custom min/maximize/close controls.
 * The window has `decorations: false`, so this replaces the native chrome. */
export function Titlebar() {
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    if (!isTauri()) return
    let unlisten: (() => void) | undefined
    void (async () => {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const win = getCurrentWindow()
      setMaximized(await win.isMaximized())
      unlisten = await win.onResized(async () => setMaximized(await win.isMaximized()))
    })()
    return () => unlisten?.()
  }, [])

  async function currentWindow() {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    return getCurrentWindow()
  }
  const minimize = async () => isTauri() && void (await currentWindow()).minimize()
  const toggleMax = async () => isTauri() && void (await currentWindow()).toggleMaximize()
  const close = async () => isTauri() && void (await currentWindow()).close()

  const ctl =
    'flex items-center justify-center w-[46px] h-full text-fg-dim transition-colors hover:bg-white/[0.06] hover:text-fg-bright'

  return (
    <div className="flex h-9 shrink-0 select-none border-b border-line-faint">
      {/* brand — aligned over the sidebar */}
      <div
        data-tauri-drag-region
        className="flex items-center gap-[10px] w-[220px] shrink-0 px-4 bg-sidebar"
      >
        <StackrIcon size={18} className="shrink-0 pointer-events-none" />
        <span className="text-[13px] font-bold tracking-[-.01em] pointer-events-none">Stackr</span>
        <span className="font-mono font-medium text-[9.5px] text-fg-dim bg-control border border-[#232834] px-[5px] py-[1px] rounded-[4px] pointer-events-none">
          v{version}
        </span>
      </div>

      {/* draggable strip over the main pane */}
      <div data-tauri-drag-region className="flex-1 bg-app" />

      {/* window controls */}
      <div className="flex bg-app">
        <button title="Minimize" onClick={() => void minimize()} className={ctl}>
          <Minus size={15} strokeWidth={2} />
        </button>
        <button title={maximized ? 'Restore' : 'Maximize'} onClick={() => void toggleMax()} className={ctl}>
          {maximized ? <Copy size={11} strokeWidth={2} /> : <Square size={11.5} strokeWidth={2} />}
        </button>
        <button
          title="Close"
          onClick={() => void close()}
          className="flex items-center justify-center w-[46px] h-full text-fg-dim transition-colors hover:bg-[#e23b35] hover:text-white"
        >
          <X size={16} strokeWidth={2} />
        </button>
      </div>
    </div>
  )
}

import { useEffect, useState } from 'react'
import { FolderOpen, HardDrive } from 'lucide-react'
import { ModalBackdrop } from './ui/ModalBackdrop'
import { useStore } from '../store/useStore'
import { isTauri, pickFolder } from '../lib/api'
import { ghostBtnAlt, inputBase, primaryBtn } from '../lib/styles'

/** First-run picker: choose where Stackr stores binaries, projects, configs and
 *  logs. Only shown on a genuinely fresh install (see backend is_first_run). */
export function FirstRunModal() {
  const rootInfo = useStore((s) => s.rootInfo)
  const chooseRoot = useStore((s) => s.chooseRoot)
  const boot = useStore((s) => s.boot)

  const [path, setPath] = useState('')
  const [busy, setBusy] = useState(false)

  // Seed the field with the default root once the info arrives.
  useEffect(() => {
    if (rootInfo) setPath(rootInfo.defaultRoot)
  }, [rootInfo])

  if (!rootInfo?.isFirstRun) return null

  const browse = async () => {
    if (!isTauri()) return
    const picked = await pickFolder(path || rootInfo.defaultRoot)
    if (picked) setPath(picked)
  }

  const confirm = async () => {
    const chosen = path.trim() || rootInfo.defaultRoot
    setBusy(true)
    const ok = await chooseRoot(chosen)
    setBusy(false)
    if (ok) void boot() // (re)load everything against the chosen root
  }

  return (
    <ModalBackdrop onClose={() => {}} dismissable={false}>
      <div
        className="w-[480px] max-w-[92vw] bg-card border border-line-input rounded-xl overflow-hidden"
        style={{ boxShadow: '0 24px 60px rgba(0,0,0,.5)' }}
      >
        <div className="p-5">
          <div className="flex items-start gap-[13px]">
            <span
              className="shrink-0 w-9 h-9 rounded-full flex items-center justify-center"
              style={{ background: 'rgba(79,140,255,.14)', color: '#4f8cff' }}
            >
              <HardDrive size={18} strokeWidth={2} />
            </span>
            <div className="min-w-0">
              <div className="text-[15px] font-semibold mb-[5px]">Choose Stackr's data folder</div>
              <div className="text-[13px] leading-[1.55] text-fg-muted">
                Stackr keeps its downloaded engines, project sites, generated configs and logs in
                one folder. Pick a location with some free space (an SSD is ideal). You can keep the
                default or choose another drive.
              </div>
            </div>
          </div>

          <div className="mt-[18px] flex gap-[10px]">
            <input
              value={path}
              onChange={(e) => setPath(e.target.value)}
              spellCheck={false}
              className={`${inputBase} flex-1`}
            />
            <button onClick={() => void browse()} className={`${ghostBtnAlt} inline-flex items-center gap-[6px] px-[14px] py-2 text-[12.5px]`}>
              <FolderOpen size={14} strokeWidth={2} />
              Browse
            </button>
          </div>
          <div className="text-[11.5px] text-fg-dim mt-2">
            This is set once. Moving it later means relocating the folder manually.
          </div>
        </div>

        <div className="px-5 py-[14px] border-t border-[#1f242f] flex justify-end">
          <button
            onClick={() => void confirm()}
            disabled={busy}
            className={`${primaryBtn} px-4 py-[9px] text-[13px] disabled:opacity-60 disabled:cursor-default`}
          >
            {busy ? 'Setting up…' : 'Use this folder'}
          </button>
        </div>
      </div>
    </ModalBackdrop>
  )
}

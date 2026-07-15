import { Check, Download, X } from 'lucide-react'
import { ModalBackdrop } from './ui/ModalBackdrop'
import { Select } from './ui/Select'
import { ProgressBar } from './ui/ProgressBar'
import { Spinner } from './ui/Spinner'
import { useStore } from '../store/useStore'
import { primaryBtn, sectionLabel } from '../lib/styles'

export function InstallModal() {
  const inst = useStore((s) => s.inst)
  const closeInstall = useStore((s) => s.closeInstall)
  const setInstVersion = useStore((s) => s.setInstVersion)
  const runInstall = useStore((s) => s.runInstall)

  if (!inst.open) return null

  const idle = inst.phase === 'idle'
  const installing = inst.phase === 'installing'
  const done = inst.phase === 'done'
  const busy = installing || done
  const pctNum = Math.round(inst.progress)
  const pct = `${pctNum}%`
  const pctColor = done ? '#3fb950' : '#7a9bff'
  const barColor = done ? '#3fb950' : '#4f7fff'

  return (
    <ModalBackdrop onClose={closeInstall} dismissable={false}>
      <div
        className="w-[440px] bg-card border border-line-input rounded-xl overflow-hidden"
        style={{ boxShadow: '0 24px 60px rgba(0,0,0,.5)' }}
      >
        {/* header */}
        <div className="px-5 py-[17px] border-b border-[#1f242f] flex items-center justify-between">
          <div className="text-[14.5px] font-semibold">Install {inst.name}</div>
          <button
            onClick={closeInstall}
            className="w-7 h-7 rounded-md bg-transparent border-none text-[#7b818f] flex items-center justify-center cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>

        {/* body */}
        <div className="p-5">
          {idle && (
            <>
              <label className={`${sectionLabel} mb-2`}>Version</label>
              <div className="mb-4">
                {inst.latest ? (
                  <div className="flex items-center justify-between px-[13px] py-[11px] bg-control border border-line-input rounded-md">
                    <span className="font-mono text-[13px] text-[#d4d9e2]">Latest stable</span>
                    <span className="text-[11px] font-semibold px-[8px] py-[2px] rounded-[20px] bg-[rgba(79,127,255,.1)] text-accent-text">
                      auto
                    </span>
                  </div>
                ) : (
                  <Select value={inst.version} onChange={setInstVersion} padRight={32}>
                    {inst.versions.map((v) => (
                      <option key={v} value={v}>
                        {v}
                      </option>
                    ))}
                  </Select>
                )}
              </div>
              <div className="flex items-center justify-between px-[13px] py-[11px] bg-inset border border-line-subtle rounded-lg">
                <span className="text-[12.5px] text-fg-muted">Download size</span>
                <span className="font-mono font-medium text-[12.5px] text-[#cfd4de]">~{inst.size}</span>
              </div>
            </>
          )}

          {busy && (
            <>
              <div className="flex items-center justify-between mb-[10px]">
                <span className="text-[12.5px] text-[#c2c7d2]">
                  {done ? 'Installation complete' : 'Downloading & installing…'}
                </span>
                <span className="font-mono font-semibold text-[12.5px]" style={{ color: pctColor }}>
                  {pct}
                </span>
              </div>
              <ProgressBar pct={pct} color={barColor} />
              <div className="font-mono font-medium text-[11.5px] text-fg-dim mt-[10px]">
                {done
                  ? `${inst.name} ${inst.version} is ready to use.`
                  : `Fetching ${inst.name} ${inst.version} (${inst.size})`}
              </div>
            </>
          )}
        </div>

        {/* footer */}
        <div className="px-5 py-[14px] border-t border-[#1f242f] flex justify-end gap-[10px]">
          {idle && (
            <>
              <button
                onClick={closeInstall}
                className="bg-transparent text-fg-muted border border-line-ghost rounded-md px-[15px] py-[9px] text-[13px] font-medium cursor-pointer transition-colors hover:bg-hover hover:text-[#cfd4de]"
              >
                Cancel
              </button>
              <button onClick={runInstall} className={`${primaryBtn} gap-2 px-4 py-[9px] text-[13px]`}>
                <Download size={15} strokeWidth={2} />
                Download &amp; Install
              </button>
            </>
          )}
          {installing && (
            <button className="inline-flex items-center gap-2 bg-[#1a1e28] text-fg-muted border border-line-input rounded-md px-4 py-[9px] text-[13px] font-semibold cursor-default">
              <Spinner size={14} strokeWidth={2.4} />
              Installing…
            </button>
          )}
          {done && (
            <button
              onClick={closeInstall}
              className="inline-flex items-center gap-2 bg-success hover:bg-success-hover text-white border-none rounded-md px-4 py-[9px] text-[13px] font-semibold cursor-pointer transition-colors"
            >
              <Check size={15} strokeWidth={2.4} />
              Done
            </button>
          )}
        </div>
      </div>
    </ModalBackdrop>
  )
}

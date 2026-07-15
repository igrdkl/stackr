import { Download, ExternalLink, Play, RotateCw, Square, Trash2 } from 'lucide-react'
import { Monogram } from './Monogram'
import { useStore } from '../../store/useStore'
import { dangerBtn, ghostBtn, primaryBtn } from '../../lib/styles'
import { hasProcess, statusVisual } from '../../lib/serviceStatus'
import type { EngineMeta, ServiceInfo, ServiceRunState } from '../../types'

const actionBtn = `${ghostBtn} inline-flex items-center gap-[6px]`
const startBtn = `${primaryBtn} gap-[6px] px-3 py-[7px] text-[12.5px]`
const uninstallBtn = `${dangerBtn} inline-flex items-center gap-[6px]`

function StatusBadge({ status }: { status: ServiceRunState }) {
  const v = statusVisual(status)
  return (
    <span
      className="inline-flex items-center gap-[6px] px-[9px] py-[3px] rounded-[20px] text-[11px] font-semibold"
      style={{ background: v.bg, color: v.color }}
    >
      <span
        className="w-[6px] h-[6px] rounded-full bg-current"
        style={{ boxShadow: v.glow ? `0 0 6px ${v.color}99` : undefined }}
      />
      {v.label}
    </span>
  )
}

/** Full-width card for one installed engine version (databases & cache). */
export function EngineVersionCard({
  svc,
  engines,
  uninstallNote,
  openAction,
}: {
  svc: ServiceInfo
  engines: EngineMeta[]
  uninstallNote: string
  /** Optional extra action shown while the service is running (e.g. "Open inbox"). */
  openAction?: { label: string; onClick: () => void }
}) {
  const startService = useStore((s) => s.startService)
  const stopService = useStore((s) => s.stopService)
  const restartService = useStore((s) => s.restartService)
  const uninstallService = useStore((s) => s.uninstallService)
  const exportDatabases = useStore((s) => s.exportDatabases)
  const askConfirm = useStore((s) => s.askConfirm)

  const meta = engines.find((e) => e.component === svc.component)
  const running = hasProcess(svc.status)
  const name = meta?.name ?? svc.name
  const isDb = ['mysql', 'mariadb', 'postgresql'].includes(svc.component)

  const onUninstall = async () => {
    // For DB engines the data dir survives uninstall, but only this engine can
    // read it — offer a SQL export first (only possible while it's running).
    const offerExport = isDb && running
    const ok = await askConfirm({
      title: `Uninstall ${name} ${svc.version}?`,
      message: isDb ? `${uninstallNote} Your database files are kept on disk.` : uninstallNote,
      confirmLabel: 'Uninstall',
      danger: true,
      checkbox: offerExport
        ? { label: 'Export all databases to a .sql backup first', defaultChecked: true }
        : undefined,
    })
    if (!ok) return
    if (offerExport && useStore.getState().confirmChecked) {
      // Export while the server is still up; abort the uninstall if it fails.
      const exported = await exportDatabases(svc.component, svc.version)
      if (!exported) return
    }
    void uninstallService(svc.id)
  }

  return (
    <div className="bg-card border border-line rounded-[10px] overflow-hidden">
      <div className="px-[18px] py-4 flex items-center gap-[15px]">
        <Monogram
          size={42}
          radius={10}
          bg={meta?.markBg ?? 'rgba(120,127,139,.14)'}
          color={meta?.markColor ?? '#9aa1ae'}
          fontSize={14}
          bold
        >
          {meta?.mark ?? '?'}
        </Monogram>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-[10px]">
            <span className="text-[14.5px] font-semibold">
              {name} <span className="font-mono font-medium">{svc.version}</span>
            </span>
            <StatusBadge status={svc.status} />
          </div>
          <div className="font-mono text-[11.5px] text-fg-dim mt-1">port {svc.port}</div>
        </div>
        <div className="flex gap-[7px]">
          {running ? (
            <button onClick={() => stopService(svc.id)} className={actionBtn}>
              <Square size={13} strokeWidth={2} />
              Stop
            </button>
          ) : (
            <button onClick={() => startService(svc.id)} className={startBtn}>
              <Play size={13} fill="currentColor" stroke="none" />
              Start
            </button>
          )}
          <button onClick={() => restartService(svc.id)} className={actionBtn}>
            <RotateCw size={13} strokeWidth={2} />
            Restart
          </button>
          {running && openAction && (
            <button onClick={openAction.onClick} className={actionBtn}>
              <ExternalLink size={13} strokeWidth={2} />
              {openAction.label}
            </button>
          )}
          <button onClick={() => void onUninstall()} className={uninstallBtn}>
            <Trash2 size={13} strokeWidth={2} />
            Uninstall
          </button>
        </div>
      </div>
    </div>
  )
}

/** Dashed "install an engine" card listing the engines for a service tab. */
export function EngineInstallCard({
  title,
  hint,
  engines,
  services,
}: {
  title: string
  hint: string
  engines: EngineMeta[]
  services: ServiceInfo[]
}) {
  const openEngineInstall = useStore((s) => s.openEngineInstall)
  const gridCls = engines.length <= 2 ? 'grid-cols-2' : 'grid-cols-3'
  const countFor = (component: string) =>
    services.filter((s) => s.component === component).length

  return (
    <div className="bg-transparent border border-dashed border-[#2a303c] rounded-[10px] px-[18px] py-[18px]">
      <div className="flex items-center gap-[15px] mb-4">
        <Monogram size={42} radius={10} bg="#13161d" color="#6f7686" fontSize={20} bold border="1px solid #232834">
          +
        </Monogram>
        <div className="flex-1 min-w-0">
          <div className="text-[14.5px] font-semibold text-[#c0c5d0]">{title}</div>
          <div className="text-[12px] text-fg-dim mt-1">{hint}</div>
        </div>
      </div>
      <div className={`grid ${gridCls} gap-[10px]`}>
        {engines.map((e) => {
          const installed = countFor(e.component)
          return (
            <button
              key={e.component}
              onClick={() => openEngineInstall(e.component)}
              className="flex items-center gap-[10px] bg-control border border-line-soft rounded-lg px-3 py-[10px] text-left cursor-pointer transition-colors hover:bg-hover2 hover:border-line-hover2"
            >
              <Monogram size={32} radius={8} bg={e.markBg} color={e.markColor} fontSize={12} bold>
                {e.mark}
              </Monogram>
              <div className="flex-1 min-w-0">
                <div className="text-[13px] font-semibold">{e.name}</div>
                <div className="text-[11px] text-fg-dim">
                  {installed ? `${installed} installed` : `${e.versions.length} versions`} · {e.size}
                </div>
              </div>
              <Download size={14} strokeWidth={2} className="text-fg-dim shrink-0" />
            </button>
          )
        })}
      </div>
    </div>
  )
}

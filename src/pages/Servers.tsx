import { Download, FileText, Info, Play, RotateCw, Square, Trash2 } from 'lucide-react'
import { ScreenHeader } from '../components/ui/ScreenHeader'
import { Monogram } from '../components/ui/Monogram'
import { useStore } from '../store/useStore'
import { ghostBtnAlt, primaryBtn } from '../lib/styles'
import { SERVER_ENGINES } from '../data/catalog'
import { hasProcess, statusVisual } from '../lib/serviceStatus'
import type { EngineMeta, ServiceInfo, ServiceRunState } from '../types'

const actionBtn = `${ghostBtnAlt} inline-flex items-center gap-2 px-4 py-[9px] text-[13px]`
const dangerActionBtn =
  'inline-flex items-center gap-2 px-4 py-[9px] text-[13px] bg-[#1a1e28] text-fg-muted border border-line-input rounded-md font-medium cursor-pointer transition-colors hover:bg-[rgba(248,81,73,.12)] hover:border-[rgba(248,81,73,.4)] hover:text-danger'

function StatusPill({ status }: { status: ServiceRunState }) {
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

function InstalledCard({ engine, server }: { engine: EngineMeta; server: ServiceInfo }) {
  const startService = useStore((s) => s.startService)
  const stopService = useStore((s) => s.stopService)
  const restartService = useStore((s) => s.restartService)
  const uninstallService = useStore((s) => s.uninstallService)
  const openConfig = useStore((s) => s.openConfig)
  const askConfirm = useStore((s) => s.askConfirm)
  const running = hasProcess(server.status)

  const onUninstall = async () => {
    const ok = await askConfirm({
      title: `Uninstall ${engine.name}?`,
      message: `${engine.name} ${server.version} will be stopped and its files removed. You can install it again later.`,
      confirmLabel: 'Uninstall',
      danger: true,
    })
    if (ok) void uninstallService(server.id)
  }

  return (
    <div className="bg-card border border-line rounded-[10px] p-[22px]">
      <div className="flex items-center gap-[13px] mb-[15px]">
        <Monogram size={44} radius={10} bg={engine.markBg} color={engine.markColor} fontSize={18} bold>
          {engine.mark}
        </Monogram>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-[10px]">
            <span className="text-[15px] font-semibold">{engine.name}</span>
            <StatusPill status={server.status} />
          </div>
          <div className="font-mono font-medium text-[11.5px] text-fg-dim mt-[2px]">
            {server.version} · port {server.port}
          </div>
        </div>
      </div>
      <div className="flex flex-wrap gap-2">
        {running ? (
          <button onClick={() => stopService(server.id)} className={actionBtn}>
            <Square size={14} strokeWidth={2} />
            Stop
          </button>
        ) : (
          <button
            onClick={() => startService(server.id)}
            className={`${primaryBtn} gap-2 px-4 py-[9px] text-[13px]`}
          >
            <Play size={14} fill="currentColor" stroke="none" />
            Start
          </button>
        )}
        <button onClick={() => restartService(server.id)} className={actionBtn}>
          <RotateCw size={14} strokeWidth={2} />
          Restart
        </button>
        <button onClick={() => void openConfig(server.component, server.version)} className={actionBtn}>
          <FileText size={14} strokeWidth={2} />
          Edit config
        </button>
        <button onClick={() => void onUninstall()} className={dangerActionBtn}>
          <Trash2 size={14} strokeWidth={2} />
          Uninstall
        </button>
      </div>
    </div>
  )
}

function InstallCard({ engine }: { engine: EngineMeta }) {
  const openInstall = useStore((s) => s.openInstall)
  return (
    <div className="bg-card border border-line rounded-[10px] p-[22px]">
      <div className="flex items-center gap-[13px] mb-[15px]">
        <Monogram size={44} radius={10} bg={engine.markBg} color={engine.markColor} fontSize={18} bold>
          {engine.mark}
        </Monogram>
        <div>
          <div className="text-[15px] font-semibold">{engine.name}</div>
          <div className="font-mono font-medium text-[11.5px] text-fg-dim mt-[2px]">
            latest stable
          </div>
        </div>
      </div>
      <p className="text-[13px] leading-[1.6] text-fg-muted mb-5">{engine.desc}</p>
      <button
        onClick={() => openInstall(engine.name, engine.component, engine.versions, engine.size, true)}
        className={
          engine.recommended
            ? `${primaryBtn} gap-2 px-4 py-[9px] text-[13px]`
            : `${ghostBtnAlt} inline-flex items-center gap-2 px-4 py-[9px] text-[13px] font-semibold`
        }
      >
        <Download size={15} strokeWidth={2} />
        Install {engine.name}
      </button>
    </div>
  )
}

export function Servers() {
  const servers = useStore((s) => s.servers)
  const anyInstalled = servers.length > 0

  return (
    <div className="max-w-[920px] w-full">
      <ScreenHeader
        title="Web Servers"
        subtitle="Choose your web server to get started — you can install both and assign one per project."
        className="mb-[26px]"
      />
      <div className="grid grid-cols-2 gap-4">
        {SERVER_ENGINES.map((engine) => {
          const server = servers.find((s) => s.component === engine.component)
          return server ? (
            <InstalledCard key={engine.component} engine={engine} server={server} />
          ) : (
            <InstallCard key={engine.component} engine={engine} />
          )
        })}
      </div>

      {!anyInstalled && (
        <div className="mt-[18px] flex items-center gap-[9px] text-[12px]">
          <Info size={14} strokeWidth={2} className="text-fg-faint shrink-0" />
          <span className="text-fg-dim">
            Nothing is installed yet. Stackr ships empty — you add only what your projects need.
          </span>
        </div>
      )}
    </div>
  )
}

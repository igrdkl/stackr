import {
  Code,
  Database,
  Folder,
  Mail,
  Server,
  Settings2,
  Terminal,
  Zap,
  type LucideIcon,
} from 'lucide-react'
import { useStore } from '../../store/useStore'
import { cn } from '../../lib/cn'
import type { TabId } from '../../types'

interface NavDef {
  id: TabId
  label: string
  Icon: LucideIcon
  badge?: string
}

const SERVICES: NavDef[] = [
  { id: 'servers', label: 'Servers', Icon: Server },
  { id: 'php', label: 'PHP', Icon: Code },
  { id: 'databases', label: 'Databases', Icon: Database },
  { id: 'cache', label: 'Cache', Icon: Zap },
  { id: 'mail', label: 'Mail', Icon: Mail },
]

const WORKSPACE: NavDef[] = [
  { id: 'projects', label: 'Projects', Icon: Folder, badge: '3' },
  { id: 'logs', label: 'Logs', Icon: Terminal },
  { id: 'settings', label: 'Settings', Icon: Settings2 },
]

function NavGroup({ label, items, className }: { label: string; items: NavDef[]; className?: string }) {
  const nav = useStore((s) => s.nav)
  const go = useStore((s) => s.go)
  const projectCount = useStore((s) => s.projects.length)
  return (
    <>
      <div
        className={cn(
          'text-[10px] font-semibold tracking-[.1em] text-fg-faint2 uppercase px-[10px] pt-[10px] pb-[6px]',
          className,
        )}
      >
        {label}
      </div>
      <div className="flex flex-col gap-[2px]">
        {items.map(({ id, label, Icon, badge }) => {
          const active = nav === id
          return (
            <div
              key={id}
              onClick={() => go(id)}
              className={cn(
                'flex items-center gap-[11px] px-[10px] py-2 rounded-[7px] cursor-pointer text-[13px] tracking-[.005em] transition-colors duration-[120ms]',
                active
                  ? 'bg-navactive text-fg-bright font-semibold'
                  : 'text-fg-dim2 font-medium hover:bg-navhover hover:text-[#c9ced8]',
              )}
            >
              <Icon size={16} strokeWidth={2} />
              <span>{label}</span>
              {badge && (
                <span className="ml-auto font-mono font-medium text-[10px] text-fg-dim bg-control border border-[#232834] px-[6px] py-px rounded-[5px]">
                  {id === 'projects' ? projectCount : badge}
                </span>
              )}
            </div>
          )
        })}
      </div>
    </>
  )
}

export function Sidebar() {
  const projects = useStore((s) => s.projects)
  const phpCount = useStore((s) => s.phpVersions.length)
  const servers = useStore((s) => s.servers)
  const runningServer = servers.find((s) => s.status === 'running')
  const up = projects.filter((p) => p.status === 'running').length
  const anyUp = up > 0 || !!runningServer

  return (
    <aside className="w-[220px] shrink-0 bg-sidebar border-r border-line-faint flex flex-col">
      {/* nav */}
      <nav className="flex-1 px-3 pt-3 pb-2 overflow-y-auto">
        <NavGroup label="Services" items={SERVICES} />
        <NavGroup label="Workspace" items={WORKSPACE} className="!pt-[18px]" />
      </nav>

      {/* bottom status */}
      <div className="p-3 border-t border-line-faint">
        <div className="flex items-start gap-[10px] px-[11px] py-[11px] bg-inset border border-[#1d212b] rounded-[9px]">
          <span
            className="w-2 h-2 rounded-full mt-[4px] shrink-0"
            style={{
              background: anyUp ? '#3fb950' : '#6b7280',
              boxShadow: `0 0 8px ${anyUp ? 'rgba(63,185,80,.55)' : 'transparent'}`,
            }}
          />
          <div className="min-w-0 leading-[1.35]">
            <div className="text-[12px] font-semibold text-[#cdd2dc]">
              {runningServer
                ? `${runningServer.name} running`
                : up > 0
                  ? 'Services running'
                  : 'All services stopped'}
            </div>
            <div className="font-mono font-medium text-[10.5px] text-fg-dim mt-[3px] space-y-[1px]">
              {runningServer && <div>Port :{runningServer.port}</div>}
              <div>
                {up} of {projects.length} sites up
              </div>
              <div>
                {phpCount} PHP {phpCount === 1 ? 'version' : 'versions'}
              </div>
            </div>
          </div>
        </div>
      </div>
    </aside>
  )
}

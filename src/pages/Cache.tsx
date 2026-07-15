import { ScreenHeader } from '../components/ui/ScreenHeader'
import { EngineInstallCard, EngineVersionCard } from '../components/ui/ServiceCards'
import { useStore } from '../store/useStore'
import { CACHE_ENGINES } from '../data/catalog'

const UNINSTALL_NOTE = 'The engine is stopped and its binaries removed.'

export function Cache() {
  const caches = useStore((s) => s.caches)
  const anyInstalled = caches.length > 0

  return (
    <div className="max-w-[920px] w-full">
      <ScreenHeader
        title="Cache"
        subtitle={
          anyInstalled
            ? 'Installed in-memory stores for sessions, queues and application caching.'
            : 'In-memory stores for sessions, queues and application caching.'
        }
        className="mb-6"
      />

      <div className="flex flex-col gap-3">
        {caches.map((svc) => (
          <EngineVersionCard key={svc.id} svc={svc} engines={CACHE_ENGINES} uninstallNote={UNINSTALL_NOTE} />
        ))}
        <EngineInstallCard
          title="Install a cache engine"
          hint="In-memory key-value stores — downloaded from the official Windows build."
          engines={CACHE_ENGINES}
          services={caches}
        />
      </div>
    </div>
  )
}

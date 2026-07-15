import { Table2 } from 'lucide-react'
import { ScreenHeader } from '../components/ui/ScreenHeader'
import { EngineInstallCard, EngineVersionCard } from '../components/ui/ServiceCards'
import { useStore } from '../store/useStore'
import { DB_ENGINES } from '../data/catalog'
import { primaryBtn } from '../lib/styles'

const UNINSTALL_NOTE =
  'The engine is stopped and its binaries removed. Databases stored in this version’s data directory are deleted with it.'

export function Databases() {
  const databases = useStore((s) => s.databases)
  const openAdminer = useStore((s) => s.openAdminer)
  const anyInstalled = databases.length > 0

  return (
    <div className="max-w-[920px] w-full">
      <ScreenHeader
        title="Databases"
        subtitle={
          anyInstalled
            ? 'Installed engines, versions and storage.'
            : 'No database engine installed yet. Pick one to enable storage for your projects.'
        }
        className="mb-6"
        right={
          anyInstalled ? (
            <button onClick={() => void openAdminer()} className={`${primaryBtn} gap-[7px] px-[15px] py-[9px] text-[13px]`}>
              <Table2 size={15} strokeWidth={2} />
              Open Adminer
            </button>
          ) : undefined
        }
      />

      <div className="flex flex-col gap-3">
        {databases.map((svc) => (
          <EngineVersionCard key={svc.id} svc={svc} engines={DB_ENGINES} uninstallNote={UNINSTALL_NOTE} />
        ))}
        <EngineInstallCard
          title="Install a database engine"
          hint="Pick an engine and version — downloaded from the official source."
          engines={DB_ENGINES}
          services={databases}
        />
      </div>
    </div>
  )
}

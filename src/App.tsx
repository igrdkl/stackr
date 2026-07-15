import { useEffect, type JSX } from 'react'
import { Titlebar } from './components/Layout/Titlebar'
import { Sidebar } from './components/Layout/Sidebar'
import { InstallModal } from './components/InstallModal'
import { NewProjectWizard } from './components/NewProjectWizard'
import { ConfigEditorModal } from './components/ConfigEditorModal'
import { AllExtensionsModal } from './components/AllExtensionsModal'
import { ConfirmDialog } from './components/ConfirmDialog'
import { FirstRunModal } from './components/FirstRunModal'
import { Toaster } from './components/ui/Toaster'
import { Servers } from './pages/Servers'
import { Php } from './pages/PHP'
import { Databases } from './pages/Databases'
import { Cache } from './pages/Cache'
import { Mail } from './pages/Mail'
import { Projects } from './pages/Projects'
import { Logs } from './pages/Logs'
import { Settings } from './pages/Settings'
import { useStore } from './store/useStore'
import type { TabId } from './types'

const SCREENS: Record<TabId, () => JSX.Element> = {
  servers: Servers,
  php: Php,
  databases: Databases,
  cache: Cache,
  mail: Mail,
  projects: Projects,
  logs: Logs,
  settings: Settings,
}

function App() {
  const nav = useStore((s) => s.nav)
  const boot = useStore((s) => s.boot)
  const checkRoot = useStore((s) => s.checkRoot)
  const checkSystem = useStore((s) => s.checkSystem)
  const checkRestoreNotice = useStore((s) => s.checkRestoreNotice)
  const loadAppVersion = useStore((s) => s.loadAppVersion)
  const checkUpdate = useStore((s) => s.checkUpdate)
  const Screen = SCREENS[nav]

  useEffect(() => {
    void checkRoot()
    void boot()
    void checkSystem()
    void checkRestoreNotice()
    void loadAppVersion()
    void checkUpdate(false) // silent boot check; surfaces in Settings if found
  }, [checkRoot, boot, checkSystem, checkRestoreNotice, loadAppVersion, checkUpdate])

  return (
    <div className="flex flex-col h-screen w-full bg-app overflow-hidden">
      <Titlebar />
      <div className="flex flex-1 min-h-0 w-full">
        <Sidebar />
        <main className="flex-1 min-w-0 flex flex-col overflow-hidden">
          <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden flex flex-col">
            <div className="flex-1 min-h-0 px-9 py-[30px] flex flex-col">
              <Screen />
            </div>
          </div>
        </main>
      </div>
      <InstallModal />
      <NewProjectWizard />
      <ConfigEditorModal />
      <AllExtensionsModal />
      <ConfirmDialog />
      <FirstRunModal />
      <Toaster />
    </div>
  )
}

export default App

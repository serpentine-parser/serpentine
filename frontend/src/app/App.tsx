import { Providers } from './providers'
import GraphPage from '../pages/GraphPage'

export default function App() {
  return (
    <Providers>
      <div className="bg-white text-gray-900 dark:bg-slate-900 dark:text-gray-100 overflow-hidden h-screen flex flex-col">
        <GraphPage />
      </div>
    </Providers>
  )
}

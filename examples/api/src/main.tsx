import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { TitleBar } from './components/title-bar'
import { error } from '@tauri-apps/plugin-log'
import { FormsGroup } from './components/api-forms'

window.addEventListener('error', async (event) => {
	try {
		await error(event.error)
	} catch (error) {
		console.error(error)
	}
})
window.addEventListener('unhandledrejection', async (event) => {
	try {
		await error(
			typeof event.reason === 'object'
				? JSON.stringify(event.reason, undefined, 4)
				: String(event.reason),
		)
	} catch (error) {
		console.error(error)
	}
})

function App() {
	return (
		<>
			<TitleBar />

			<h1 className="title">Verso (Servo) + Tauri!</h1>
			<FormsGroup />
		</>
	)
}

createRoot(document.getElementById('root')!).render(
	<StrictMode>
		<App />
	</StrictMode>,
)

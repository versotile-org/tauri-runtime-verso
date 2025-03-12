import { StrictMode, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { TitleBar } from './title-bar'
import { invoke } from '@tauri-apps/api/core'

function App() {
	const [name, setName] = useState('')
	const [message, setMessage] = useState('')

	return (
		<>
			<TitleBar />

			<h1 className="title">Welcome to Tauri!</h1>

			<div className="form-and-message">
				<form
					id="form"
					onSubmit={async (ev) => {
						ev.preventDefault()
						const newMessage = await invoke<string>('greet', { name })
						setMessage(newMessage)
					}}
				>
					<input
						id="name"
						placeholder="Enter a name..."
						value={name}
						onChange={(e) => setName(e.target.value)}
					/>
					<button>Greet</button>
				</form>
				<div id="message">{message}</div>
			</div>
		</>
	)
}

createRoot(document.getElementById('root')!).render(
	<StrictMode>
		<App />
	</StrictMode>,
)

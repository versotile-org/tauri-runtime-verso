import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { info } from '@tauri-apps/plugin-log'
import { getName } from '@tauri-apps/api/app'
import { resolve, appLogDir } from '@tauri-apps/api/path'
import { openPath } from '@tauri-apps/plugin-opener'

import './api-forms.css'

export function HelloWorld() {
	const [name, setName] = useState('')
	const [message, setMessage] = useState('')
	return (
		<div className="form-and-message">
			<form
				onSubmit={async (ev) => {
					ev.preventDefault()
					const newMessage = await invoke<string>('greet', { name })
					setMessage(newMessage)
				}}
			>
				<input
					placeholder="Enter a name..."
					value={name}
					onChange={(e) => setName(e.target.value)}
				/>
				<button>Greet</button>
			</form>
			<div className="message">{message}</div>
		</div>
	)
}

export function LoggingExample() {
	const [name, setName] = useState('')
	return (
		<div className="form-and-message logging-example">
			<form
				onSubmit={async (ev) => {
					ev.preventDefault()
					await info(name)
				}}
			>
				<input
					placeholder="Enter something to log..."
					value={name}
					onChange={(e) => setName(e.target.value)}
				/>
				<button>Log it!</button>
			</form>
			<button
				onClick={async () => openPath(await resolve(await appLogDir(), `${await getName()}.log`))}
			>
				Open the log file
			</button>
		</div>
	)
}

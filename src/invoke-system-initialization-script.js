// This file is copied and modified from Tauri with a few modifications
// - Changed `processIpcMessage` to always return a string so we can put it inside of http request header
// - Changed custom protocol IPC to use header instead of body since we can't get the body in Servo yet
//
// > ipc-protocol.js: https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/scripts/ipc-protocol.js
// > process-ipc-message-fn.js: https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/scripts/process-ipc-message-fn.js

;(function () {
	const processIpcMessage = function (message) {
		// if (message instanceof ArrayBuffer || ArrayBuffer.isView(message) || Array.isArray(message)) {
		// 	return {
		// 		contentType: 'application/octet-stream',
		// 		data: message,
		// 	}
		// } else {
		const data = JSON.stringify(message, (_k, val) => {
			// if this value changes, make sure to update it in:
			// 1. ipc.js
			// 2. core.ts
			const SERIALIZE_TO_IPC_FN = '__TAURI_TO_IPC_KEY__'

			if (val instanceof Map) {
				return Object.fromEntries(val.entries())
			} else if (val instanceof Uint8Array) {
				return Array.from(val)
			} else if (val instanceof ArrayBuffer) {
				return Array.from(new Uint8Array(val))
			} else if (typeof val === 'object' && val !== null && SERIALIZE_TO_IPC_FN in val) {
				return val[SERIALIZE_TO_IPC_FN]()
			} else if (
				val instanceof Object &&
				'__TAURI_CHANNEL_MARKER__' in val &&
				typeof val.id === 'number'
			) {
				return `__CHANNEL__:${val.id}`
			} else {
				return val
			}
		})

		return {
			contentType: 'application/json',
			data,
		}
		// }
	}

	/**
	 * A runtime generated key to ensure an IPC call comes from an initialized frame.
	 *
	 * This is declared outside the `window.__TAURI_INVOKE__` definition to prevent
	 * the key from being leaked by `window.__TAURI_INVOKE__.toString()`.
	 */
	const __TAURI_INVOKE_KEY__ = __INVOKE_KEY__

	let customProtocolIpcFailed = false

	function sendIpcMessage(message) {
		const { cmd, callback, error, payload, options } = message

		if (!customProtocolIpcFailed) {
			const { contentType, data } = processIpcMessage(payload)
			fetch(window.__TAURI_INTERNALS__.convertFileSrc(cmd, 'ipc'), {
				method: 'POST',
				// body: data,
				headers: {
					'Content-Type': contentType,
					'Tauri-Callback': callback,
					'Tauri-Error': error,
					'Tauri-Invoke-Key': __TAURI_INVOKE_KEY__,
					'Tauri-VersoRuntime-Invoke-Body': data,
					...((options && options.headers) || {}),
				},
			})
				.then((response) => {
					const cb = response.headers.get('Tauri-Response') === 'ok' ? callback : error
					// we need to split here because on Android the content-type gets duplicated
					switch ((response.headers.get('content-type') || '').split(',')[0]) {
						case 'application/json':
							return response.json().then((r) => [cb, r])
						case 'text/plain':
							return response.text().then((r) => [cb, r])
						default:
							return response.arrayBuffer().then((r) => [cb, r])
					}
				})
				.then(([cb, data]) => {
					if (window[`_${cb}`]) {
						window[`_${cb}`](data)
					} else {
						console.warn(
							`[TAURI] Couldn't find callback id {cb} in window. This might happen when the app is reloaded while Rust is running an asynchronous operation.`
						)
					}
				})
				.catch((e) => {
					console.warn(
						'IPC custom protocol failed, Tauri will now use the postMessage interface instead',
						e
					)
					// failed to use the custom protocol IPC (either the webview blocked a custom protocol or it was a CSP error)
					// so we need to fallback to the postMessage interface
					customProtocolIpcFailed = true
					sendIpcMessage(message)
				})
		} else {
			// otherwise use the postMessage interface
			const { data } = processIpcMessage({
				cmd,
				callback,
				error,
				options: {
					...options,
					customProtocolIpcBlocked: customProtocolIpcFailed,
				},
				payload,
				__TAURI_INVOKE_KEY__,
			})
			window.ipc.postMessage(data)
		}
	}

	Object.defineProperty(window.__TAURI_INTERNALS__, 'postMessage', {
		value: sendIpcMessage,
	})
})()

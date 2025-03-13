import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

import './title-bar.css'

export function TitleBar() {
	return (
		<div className="title-bar" data-tauri-drag-region>
			<button onClick={() => getCurrentWebviewWindow().minimize()}>-</button>
			<button onClick={() => getCurrentWebviewWindow().toggleMaximize()}>▢</button>
			<button onClick={() => getCurrentWebviewWindow().close()} className="close-button">
				×
			</button>
		</div>
	)
}

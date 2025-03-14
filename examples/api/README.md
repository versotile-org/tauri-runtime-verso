To get started with this example

1. Clone [`verso`](https://github.com/versotile-org/verso) to this repo's parent directory
2. Build verso with `cargo build` (You might need to install a few other dependencies, please follow the [instruction](https://github.com/versotile-org/verso#getting-started) there)
3. Run `pnpm install` to install dependencies
4. Run `pnpm web:dev` to start the dev vite server
5. Run `pnpm dev` to run this Tauri app (You'll also need the [prerequisites](https://tauri.app/start/prerequisites/) of Tauri) (Also you need to run this command in this directory for the relative paths to work)

You can take a look at `build.rs` and `tauri.conf.json` to see where it copies the versoview from, and change it to match your situation

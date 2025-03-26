# Tauri Runtime Verso

Use Verso as the backend for Tauri

> Currently still working in progress

## Usage

```rust
use tauri::Manager;
use tauri_runtime_verso::{set_verso_path, set_verso_resource_directory, INVOKE_SYSTEM_SCRIPTS};

fn main() {
    // You need to set this to the path of the versoview executable
    // before creating any of the webview windows
    set_verso_path("../verso/target/debug/versoview");
    // Set this to verso/servo's resources directory before creating any of the webview windows
    // this is optional but recommended, this directory will include very important things
    // like user agent stylesheet
    set_verso_resource_directory("../verso/resources");
    tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
        // Make sure to do this or some of the commands will not work
        .invoke_system(INVOKE_SYSTEM_SCRIPTS.to_owned())
        .run(tauri::generate_context!())
        .unwrap();
}
```

For more, take a look at the [hello world example](examples/helloworld), or a more sophisticated [api example](examples/api) show casing how you can use [`react`](https://react.dev/) in it and how to bundle the versoview executable and resource directory with `tauri-cli`'s bundler feature so you can actually easily distribute your app

Also, you can checkout the [documentation](https://versotile-org.github.io/tauri-runtime-verso/tauri_runtime_verso)

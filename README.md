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
    // this is optional but recommanded, this directory will include very important things
    // like user agent stylesheet
    set_verso_resource_directory("../verso/resources");
    tauri::Builder::<tauri_runtime_verso::VersoRuntime>::new()
        // Make sure to do this or some of the commands will not work
        .invoke_system(INVOKE_SYSTEM_SCRIPTS.to_owned())
        .run(tauri::generate_context!())
        .unwrap();
}
```

For more, take a look at the [hello world example](examples/helloworld)

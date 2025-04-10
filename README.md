# Tauri Runtime Verso

A tauri runtime to replace the backend with [Verso](https://github.com/versotile-org/verso)

> Currently still working in progress

## Usage

To get started, you need to add this crate to your project, and use `default-feature = false` on `tauri` to disable the `wry` feature

```diff
- tauri = { version = "2", features = [] }
+ tauri = { version = "2", default-features = false, features = ["common-controls-v6"] }
+ tauri-runtime-verso = { git = "https://github.com/versotile-org/tauri-runtime-verso.git" }
```

And then setup the code like this:

```rust
use tauri_runtime_verso::{
    INVOKE_SYSTEM_SCRIPTS, VersoRuntime, set_verso_path, set_verso_resource_directory,
};

fn main() {
    // You need to set this to the path of the versoview executable
    // before creating any of the webview windows
    set_verso_path("../verso/target/debug/versoview");
    // Set this to verso/servo's resources directory before creating any of the webview windows
    // this is optional but recommended, this directory will include very important things
    // like user agent stylesheet
    set_verso_resource_directory("../verso/resources");
    // Set `tauri::Builder`'s generic to `VersoRuntime`
    tauri::Builder::<VersoRuntime>::new()
        // Make sure to do this or some of the commands will not work
        .invoke_system(INVOKE_SYSTEM_SCRIPTS.to_owned())
        .run(tauri::generate_context!())
        .unwrap();
}
```

For more, take a look at the [hello world example](examples/helloworld), or a more sophisticated [api example](examples/api) show casing how you can use [`react`](https://react.dev/) in it and how to bundle the versoview executable and resource directory with `tauri-cli`'s bundler feature so you can actually easily distribute your app

Also, you can checkout the [documentation](https://versotile-org.github.io/tauri-runtime-verso/tauri_runtime_verso)

# Tauri Runtime Verso

A tauri runtime to replace the backend with [Verso](https://github.com/versotile-org/verso)

> Currently still working in progress

## Usage

To get started, you need to add this crate to your project, and use `default-feature = false` on `tauri` to disable the `wry` feature

```diff
  [build-dependencies]
  tauri-build = "2"
+ tauri-runtime-verso-build = { git = "https://github.com/versotile-org/tauri-runtime-verso.git" }

  [dependencies]
- tauri = { version = "2", features = [] }
+ tauri = { version = "2", default-features = false, features = ["common-controls-v6"] }
+ tauri-runtime-verso = { git = "https://github.com/versotile-org/tauri-runtime-verso.git" }
```

In your build script, add the `tauri-runtime-verso-build` script, which will download the pre-built `versoview` to `versoview/versoview-{target-triple}`

```diff
fn main() {
+   tauri_runtime_verso_build::get_verso_as_external_bin().unwrap();
    tauri_build::build();
}
```

Then add the downloaded executable to your tauri config file (`tauri.conf.json`)

```diff
  {
+   "bundle": {
+     "externalBin": [
+       "versoview/versoview"
+     ]
+   }
  }
```

Finally, setup the code like this:

```rust
use tauri_runtime_verso::{INVOKE_SYSTEM_SCRIPTS, VersoRuntime};

fn main() {
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

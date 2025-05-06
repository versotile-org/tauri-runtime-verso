# Tauri Runtime Verso

A tauri runtime to replace the backend with [Verso](https://github.com/tauri-apps/verso)

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

> Note we currently only have pre-built `versoview` for x64 Linux, Windows, MacOS and arm64 MacOS, also the download might take a bit of time if you have a slow internet connection

```diff
fn main() {
+   tauri_runtime_verso_build::get_verso_as_external_bin().unwrap();
    tauri_build::build();
}
```

Then add the downloaded executable to your tauri config file (`tauri.conf.json`) as an external binary file

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

```diff
fn main() {
-   tauri::Builder::new()
+   tauri_runtime_verso::builder()
        .run(tauri::generate_context!())
        .unwrap();
}
```

For more, take a look at the [hello world example](examples/helloworld), or a more sophisticated [api example](examples/api) show casing how you can use [`React`](https://react.dev/) in it and how to bundle the versoview executable and resource directory with `tauri-cli`'s bundler feature so you can actually easily distribute your app

Also, you can checkout the [documentation](https://versotile-org.github.io/tauri-runtime-verso/tauri_runtime_verso)

### Common Problems

#### No such file or directory on Linux

This error means either the path you set through `set_verso_path` is wrong (this should not be a problem if you're using the `externalBin` setup from the [Usage](#usage)) or the `versoview` exectuable requires a more recent version of glibc that your system doesn't have, in this case, you'll need to either update your linux distro or build `versoview` yourself

## Tips

### Devtools

Since Verso doesn't have a devtools built-in, you'll need to use the one from the Firefox, first put in this in your code

```rust
// This will make the webviews created afters this open up a devtools server on this port,
// setting it to 0 for a random port
tauri_runtime_verso::set_verso_devtools_port(1234);
```

Then go to `about:debugging` in Firefox and connect to `localhost:1234` there

## Known limitations

Currently, only the app wide menus on macOS are supported, per window menus are not supported yet

For more, checkout the [documentation](https://versotile-org.github.io/tauri-runtime-verso/tauri_runtime_verso)

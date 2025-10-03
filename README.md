# Tauri Runtime Verso

A tauri runtime to replace the backend with [Verso](https://github.com/tauri-apps/verso)

## What is Verso

[Verso](https://github.com/tauri-apps/verso) is a browser built on top of the [Servo](https://servo.org/) browser engine, and in our case, we use it as a webview. The reason for this is because the Servo embedding APIs are quite low level and requires the user (embedder) to send everything about the system to drive it, and with Verso, we provide Servo with these things under the hood, then expose some higher level APIs so that you can just let it run and only control it when you need something, this is similar to the [windowed vs visual hosting mode](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/windowed-vs-visual-hosting) of [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2) except for when using Servo directly, you'll need to pass in more than just user inputs

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

For more, take a look at the [hello world example](examples/helloworld), or a more sophisticated [api example](examples/api) show casing how you can use [`React`](https://react.dev/), tauri plugins, and tray icons with it

Also, you can checkout the [documentation](https://versotile-org.github.io/tauri-runtime-verso/tauri_runtime_verso)

### Common Problems

#### No such file or directory on Linux

This error means either the path you set through `set_verso_path` is wrong (this should not be a problem if you're using the `externalBin` setup from the [Usage](#usage)) or the `versoview` exectuable requires a more recent version of glibc that your system doesn't have, in this case, you'll need to either update your linux distro or build `versoview` yourself

#### Failed to select a version for ...

```
error: failed to select a version for `tauri-utils`.
    ... required by package `tauri-runtime-verso v0.1.0 (https://github.com/versotile-org/tauri-runtime-verso.git#ad4815ef)`
    ... which satisfies git dependency `tauri-runtime-verso` of package `testo v0.1.0 (/src/testo/src-tauri)`
versions that meet the requirements `=2.6.0` are: 2.6.0
```

> https://github.com/versotile-org/tauri-runtime-verso/issues/19

To solve this, relax the version selection in your `Cargo.toml` file (e.g. `tauri = "2"` instead of `tauri = "2.8.5"`) and then run `cargo update`

Since we rely on some unstable/non-semver compatible features of Tauri, we pinned a few tauri crates' versions, this sometimes causes package selection conflicts, and usually a `cargo update` would fix that

If you rely on a feature in the latest Tauri which the Verso runtime doesn't support yet, file an [issue](https://github.com/versotile-org/tauri-runtime-verso/issues), or a [pull request](https://github.com/versotile-org/tauri-runtime-verso/pulls) would be much appreciated

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

### Security

We currently hard coded the `Origin` header for the custom protocol IPC to work, but this means Tauri won't be able to check for if the URL is a remote URL or a local one for the capabilities, so right now, please don't use this to load arbitrary websites if you have related settings

### Menus

Currently, only the app wide menus on macOS are supported, per window menus are not supported yet

### Mobile

We currently only support Linux, Windows, MacOS, so no mobile (e.g. Android / iOS) support yet

If your app targets both desktop and mobile and want to use this runtime only on desktop, you'll need to:

- Include the crates in this repo only in desktop target dependencies
- Enable `wry` feature on `tauri` when targeting mobile
- Use `tauri::Builder::new()` instead of `tauri_runtime_verso::builder()` on mobile
- Add checks to only run `get_verso_as_external_bin` when targeting desktop
- Add `externalBin` only for desktop platforms, either through [tauri conf file for each platform](https://tauri.app/reference/config/#platform-specific-configuration) or `--config` flag to target a different configuration file that contains the `externalBin` field

---

For more, checkout the [documentation](https://versotile-org.github.io/tauri-runtime-verso/tauri_runtime_verso)

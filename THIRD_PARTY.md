# Third-party components

nxrt-frontend is released into the public domain ([`LICENSE`](LICENSE)). The app it builds includes or links the
components below; each keeps its own license. Nothing here or in the app includes Nintendo game code, assets, fonts or
keys: players supply their own copies.

## In the app

| Component                                                                                                    | What for                            | License                                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------ | ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| [nxrt](https://github.com/fedebuonco/nxrt) (its source, in `Contents/Resources/nxrt`)                        | creating recomps without a checkout | public domain (Unlicense); its own third-party components in its [`THIRD_PARTY.md`](https://github.com/fedebuonco/nxrt/blob/master/THIRD_PARTY.md) |
| [Tauri 2](https://tauri.app) and its crates                                                                  | the window and the app              | MIT or Apache 2.0                                                                                                                                  |
| [SDL 3](https://libsdl.org) via [sdl3-sys](https://crates.io/crates/sdl3-sys) (built from source, linked in) | controllers                         | zlib                                                                                                                                               |
| Rust crates (serde, toml, tracing, rfd, base64, dirs, and their dependencies)                                | the backend                         | MIT, Apache 2.0, zlib, BSD, Unicode 3.0 or Unlicense, as each crate states                                                                         |
| A few crates under MPL 2.0 (pulled in by Tauri)                                                              | the backend                         | MPL 2.0: their unmodified source is on [crates.io](https://crates.io)                                                                              |

`cargo metadata` in `src-tauri/` lists every crate with its license.

## Development only

Not shipped in the app: Vite, Vitest, Playwright, ESLint, Prettier (MIT / Apache 2.0), installed by `npm install`.

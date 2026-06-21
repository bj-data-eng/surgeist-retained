# surgeist-retained

Retained semantic UI model primitives for Surgeist: stable identity, tree handles, and retained state contracts.

## API Artifact

The committed API coordination artifact lives at `api/public-api.txt`.

Refresh it explicitly with:

```sh
cargo run --manifest-path api/generator/Cargo.toml
```

API refresh tooling is command-only and must not run as part of normal `cargo test`.

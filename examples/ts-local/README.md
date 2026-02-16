# TS Local Example

Minimal TypeScript local embedding example for `nervusdb-node`.

## Run

```bash
cargo build --manifest-path nervusdb-node/Cargo.toml --release
npm --prefix examples/ts-local ci
npm --prefix examples/ts-local run smoke
```

The example validates `open -> executeWrite -> query -> beginWrite/commit -> close`.

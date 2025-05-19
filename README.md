# WaspInput
Input tool for the OldSchool Runescape's OSClient for Simba.

This allows you to send fake input to the game client while using your computer.

You can build it with:
```
cargo build
```

To build for specific targets:
```
//32 bits windows
cargo build --target=i686-pc-windows-msvc
//64 bits windows
cargo build --target=x86_64-pc-windows-msvc
```

You can find auto-built binaries on the [releases](https://github.com/Torwent/wasp-input/releases) page.

This is quite complex and the built plugin has 2 sides to it, one that runs exclusively on Simba, another one that runs exclusively on the client and some code runs on both sides.

`lib.rs` and `target.rs` code runs exclusively on Simba.
`client.rs` and `graphics.rs` code run exclusively on the client.

The rest of the files have code that runs on both.

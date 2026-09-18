# confkit
> **Note** This repository is primarily for easy access to the source code for alteration.
> The readme below is identical to that of the crates.io page, but without the cargo docs,
> For actual use or learning the library, I encourage you to check out the main page here:
> https://crates.io/crates/confkit

Confkit is an opinionated Rust library for easily creating and loading TOML configuration files with minimal boilerplate.

`confkit` is designed for applications that want a simple configuration system without needing to manually handle configuration paths, file creation, and serialization.

> **Note:** `confkit` currently supports Unix-like systems only.

## Features

* Automatically locates configuration files using XDG directories.
* Creates missing configuration directories and files.
* Generates configuration files from `Default` values.
* Supports field-level default values.
* Supports inline comments in generated configuration files.
* Supports nested configuration structs.
* Allows configuration directories and filenames to be customized.
* Loads configuration files directly into your Rust structs using Serde.

## Getting Started

Add `confkit` to your project:

```toml
[dependencies]
confkit = "0.1"
```

Then derive `Config` on your configuration struct:

```rust
use confkit::Config;

#[derive(Config)]
struct MyConfig {
    username: String,

    #[detail(
        default = true,
        comment("Whether notifications are enabled")
    )]
    notifications: bool,
}

fn main() -> Result<(), confkit::error_handling::ConfigError> {
    let config = MyConfig::init()?;

    println!("Hello, {}!", config.username);

    Ok(())
}
```

The first time `MyConfig::init()` is called, `confkit` creates the configuration directory and generates a `config.toml` using the default values.

For example:

```toml
username = ""

# Whether notifications are enabled
notifications = true
```

On subsequent runs, the existing configuration is loaded automatically.

## `init()` vs `load()`

`init()` is intended for the common case where the configuration file may not exist yet.

```rust
let config = MyConfig::init()?;
```

It will:

1. Locate the configuration directory.
2. Create it if necessary.
3. Generate a configuration file if one does not exist.
4. Load the configuration.

If the configuration file must already exist, use `load()` instead:

```rust
let config = MyConfig::load()?;
```

`load()` will return an error if the configuration directory or file does not exist.

## Defaults

By default, each fields default value is provided by `Default::default()`.

This can be modified on a per field basis using `#[detail(default = ...)]`,
the struct's `Default` implimentation will also be changed accordingly.

```rust
#[derive(Config)]
struct MyConfig {
    username: String,

    #[detail(default = 8080)]
    port: u16,

    #[detail(default = true)]
    enabled: bool,
}
```

## Comments

Comments can be added to individual fields using `#[detail(comment("..."))]`:

```rust
#[derive(Config)]
struct MyConfig {
    #[detail(comment("The username displayed by the application"))]
    username: String,

    #[detail(
        default = 8080,
        comment("Port used by the server")
    )]
    port: u16,
}
```

Generated configuration:

```toml
username = "" # The username displayed by the application

port = 8080 # Port used by the server
```

Comments can also be attached to nested configuration structs.

## Nested Configuration

Configuration structs can contain other configuration structs.

Use `#[detail(nested)]` when comments from the nested struct should be included in the generated configuration:

```rust
#[derive(Config)]
struct ServerConfig {
    #[detail(default = 8080, comment("Port used by the server"))]
    port: u16,
}

#[derive(Config)]
struct MyConfig {
    #[detail(comment("Server configuration"), nested)]
    server: ServerConfig,
}
```

This produces a nested TOML table while preserving the comments defined by the nested struct.

Nested structs can be nested to any depth.

## Configuration Location

Confkit resolves configuration paths from the user's XDG configuration directory.

The configuration directory defaults to the lowercase name of the root configuration struct, and the filename defaults to `config.toml`.

For example:

```rust
#[derive(Config)]
struct MyApplication {
    // ...
}
```

will use:

```text
~/.config/myapplication/config.toml
```

The location can be customized with `#[config(...)]`.

For example:

```rust
#[derive(Config)]
#[config(dir = "my-app", file = "settings.toml")]
struct MyConfig {
    // ...
}
```

will use:

```text
~/.config/my-app/settings.toml
```

## Generating Configuration Without Loading

If you only need to generate the configuration contents, `generate_config_file()` can be used directly:

```rust
let contents = MyConfig::generate_config_file()?;

println!("{contents}");
```

This generates the configuration contents without writing anything to disk.

## License

Licensed under either of:

* Apache License, Version 2.0
* MIT License

at your option.

See [`LICENSE-APACHE`](LICENSE-APACHE) and [`LICENSE-MIT`](LICENSE-MIT) for details.

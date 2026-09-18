//! # Confkit
//!
//! Confkit is an opinionated library aimed at creating and loading TOML configuration files easily and with minimal boilerplate.
//!
//! Currently, Confkit only supports Unix-like systems.
//!
//! ## Features
//!
//! - Automatically locates configuration files using XDG directories.
//! - Creates missing configuration directories and files.
//! - Generates configuration files from 'Default' values.
//! - Supports field level comments and defaults.
//! - Supports nested configuration structs.
//!
//! ## Example
//!
//! ```
//! use confkit::Config;
//!
//! #[derive(Config)]
//! struct MyConfig {
//!     username: String,
//!     #[detail(default = true, comment("Enables Notifications"))]
//!     notifications: bool,
//! }
//!
//! # fn main() -> Result<(), confkit::error_handling::ConfigError> {
//! let config = MyConfig::init()?;
//! # Ok(())
//! # }
//! ```
//!
//! `init()` automatically creates the configuration directory and file
//! when they do not already exist, then loads the resulting configuration.
//!
//! If you only want to load an existing configuration, use `MyConfig::load()` instead.
//!
//! ## Configuration
//! 
//! Configuration behavior can be customized using '#[config(...)]'
//! on the struct and '#[detail(...)]' on individual fields.
//!
//! See [`Config`] and [`confkit_derive::derive`] for details.
//!
//! ## Errors
//!
//! Confkit returns [`ConfigError`] when configuration files cannot be located,
//! created, read, or deserialized.

// Re-exports
extern crate self as confkit;
/// Implements the [`Config`] trait on the derived struct.
///
/// This is the intended entry point for Confkit. It is normally used as #[derive(Config)].
/// For information on the functions this implements see [`Config`].
///
/// ## Attributes
///
/// `#[config(...)]`
///
/// Configures the location of the configuration files.
/// This should only be applied to the struct itself, and does not need to be applied to children.
///
/// Available options:
///
/// - `dir = "..."` changes the configuration directory.
/// - `file = "..."` changes the configuration filename.
///
/// If omitted, the directory defaults to the struct's name in lowercase, and the filename to 'config.toml'.
///
/// Paths, including custom paths, are always resolved relative to the user's XDG config directory.
///
/// `#[detail(...)]`
///
/// Configures default values and inline comments for individual fields.
/// 
/// Available options:
///
/// - `comment("...")` applies an inline comment to the field its applied to.
/// - `default = ...` changes the value a field will be when a config file is generated.
/// - `nested` must be applied to nested structs if you wish to give them comments.
///
/// Comments will always appear as inline, and can be applied to fields including nested structs, which would comment a header.
///
/// If no default value is specified, a fields value will be taken from [`Default::default`].
/// Specifying a default, overrides that value for generated config files and the generated implimentation of Default.
///
/// The nested option tells the derive macro to recursively collect comments from the nested struct.
/// It is only required when the nested struct contains comments you wish to appear in the generated configuration file.
/// Nested structs may be nested to any depth.
///
/// ## Example
///
/// ```
/// use confkit::Config;
///
/// #[derive(Config)]
/// #[config(dir = "example_config", file = "configuration.toml")]
/// struct User {
///     #[detail(comment("Your username"))]
///     username: String,
///     #[detail(nested)]
///     user_preferences: Preferences,
/// }
///
/// #[derive(Config)]
/// struct Preferences {
///     #[detail(default = true, comment("Enables Notifications"))]
///     notifications: bool,
/// }
/// ```
///
/// When the above example is created such as by calling `User::init()`,
/// the resulting configuration file would be located in `~/.config/example_config/configuration.toml`.
/// 
/// The resulting configuration file's contents would be:
///
/// ```
/// username = "" # Your username
///
/// [user_preferences]
/// notifications = true # Enables Notifications
/// ````

pub use confkit_derive::derive as Config;
pub use config_file::Config as Config;

#[doc(hidden)]
pub mod private {
    pub use serde;
    pub use toml_edit;
    
    pub struct Comment {
        pub key: &'static str,
        pub text: Option<&'static str>,
        pub children: Vec<Comment>,
    }
}

pub mod config_file {
    // External imports
    use serde::{de::DeserializeOwned, Serialize};
    use std::{
        path::Path,
        fs,
    };
    
    /// Provides configuration file creation and loading for a type.
    ///
    /// This trait is normally implemented automatically using
    /// `#[derive(Config)]`.
    pub trait Config :Sized + Serialize + DeserializeOwned {
        /// Loads an existing configuration file from the user's XDG config directory.
        /// 
        /// Use this function when the configuration file must already exist, otherwise use `init()`
        ///
        /// # Errors
        /// 
        /// Returns an error if:
        ///
        /// - the configuration directory does not exist,
        /// - the configuration file does not exist, or
        /// - the configuration file cannot be read or fails to be deserialized.
        fn load() -> Result<Self, ConfigError>;
        #[doc(hidden)]
        fn comments() -> Vec<Comment>;
        /// Generates the contents of a new configuration file.
        /// 
        /// By default all fields use the values provided by [`Default::default`],
        /// this can be customized using attributes. See [`confkit_derive::derive`] for details.
        /// Inline comments can also be added using attributes.
        ///
        /// This function only generates the contents of a configuration file, it does not write anything to the user's disk.
        /// If you want Confkit to create and load the configuration automatically, use `init()` instead. 
        fn generate_config_file() -> Result<String, ConfigError>;
        /// Loads a configuration file if it exists, otherwise generates one
        /// before loading it.
        ///
        /// This is the main intended way to use the crate. It handles locating,
        /// creating, and loading the configuration automatically.
        ///
        /// If you require loading only if a configuration file already exists, use `load()` instead.
        ///
        /// By default, the configuration is stored in a directory named after
        /// the struct in lowercase, with a file named `config.toml`. These defaults,
        /// along with generated values and comments, can be customized using
        /// attributes. See [`confkit_derive::derive`] for details.
        ///
        /// # Errors
        ///
        /// Returns an error if:
        ///
        /// - the configuration file cannot be read or fails to be deserialized,
        /// - a new directory or file is unable to be created, or
        /// - an invalid custom path is specified.
        fn init() -> Result<Self, ConfigError>;
    }
    
    // Internal imports
    use super::error_handling::ConfigError;
    use super::private::Comment;
    use config_generation::generate_config_file;
    use config_locating::{get_xdg_config_dir, validate_config_path, MissingConfig};
    
    #[doc(hidden)]
    pub fn init<T>(dir_name :&str, file_name :&str, file_contents :String, comments :Vec<Comment>) -> Result<T, ConfigError> 
        where T:DeserializeOwned {
            
        let mut path = get_xdg_config_dir()?;
        path.push(dir_name);
        path.push(file_name);
        
        if let Some(issue) = validate_config_path(&path)? {
            match issue {
                MissingConfig::Directory => {
                    let config_path = path.parent().ok_or_else(|| ConfigError::new(
                        format!("failed to determine config directory from path: {:?}", path)    
                    ))?;
                    
                    fs::create_dir_all(config_path).map_err(|e| ConfigError::new(
                        format!("failed to create new config directory '{:?}': {}", config_path, e)
                    ))?;
                },
                MissingConfig::File => {},
            }
            fs::write(&path, generate_config_file(file_contents, comments)?).map_err(|e|
                ConfigError::new(format!("failed to write new config file to '{:?}': {}", path, e)
            ))?;
        };
        
        load_config_file(&path)
    }
    
    #[doc(hidden)]
    pub fn load<T>(config_dir_name :&str, config_file_name :&str) -> Result<T, ConfigError> where T:DeserializeOwned {
        let mut path = get_xdg_config_dir()?;
        path.push(config_dir_name);
        path.push(config_file_name);
        
        if let Some(issue) = validate_config_path(&path.as_path())? {
            let error_message = match issue {
                MissingConfig::Directory => format!("unable to find directory '{}' within xdg config home", config_dir_name),
                MissingConfig::File => format!("unable to find file '{}' within '{}'", config_file_name, config_dir_name),
            };
            return Err(ConfigError::new(error_message));
        };
        
        Ok(load_config_file(&path)?)
    }
    
    #[doc(hidden)]
    fn load_config_file<T>(path :&Path) -> Result<T, ConfigError> where T:DeserializeOwned, {
        let file_contents = fs::read_to_string(path).map_err(|e| ConfigError::new(
            format!("failed to read config file {}: {}", path.display(), e)
        ))?; 
        
        let config = toml_edit::de::from_str(&file_contents).map_err(|e| ConfigError::new(
            format!("failed to deserialize config file {}: {}", path.display(), e)
        ))?;
        
        Ok(config)
    }
    
    #[doc(hidden)]
    pub mod config_locating {
        // Imports
        use std::path::{Path, PathBuf};
        use xdg::BaseDirectories;
        use crate::error_handling::ConfigError;
        
        #[derive(Debug)]
        pub enum MissingConfig {
            Directory,
            File,
        }
        
        pub fn get_xdg_config_dir() -> Result<PathBuf, ConfigError> {
            let path = BaseDirectories::new().config_home.ok_or_else(|| ConfigError::new(
                format!("unable to find the users xdg config home")
            ))?;
            
            Ok(path)
        }
        
        pub fn validate_config_path(path :&Path) -> Result<Option<MissingConfig>, ConfigError> {
            let config_dir = path.parent().ok_or_else(|| ConfigError::new(
                format!("failed to determine config directory from path: {:?}", path)
            ))?;
            
            if !config_dir.is_dir() {
                return Ok(Some(MissingConfig::Directory))
            };
            
            if !path.is_file() {
                return Ok(Some(MissingConfig::File))
            };
            
            Ok(None)
        }
    }
    
    // This module contains the functions used to generate new config files
    #[doc(hidden)]
    pub mod config_generation {
        // Imports
        use std::str::FromStr;
        use toml_edit::{DocumentMut, Item};
        use crate::{error_handling::ConfigError, private::Comment};
        
        // This function is the intended method to create config files using Confkit, it calls all other necessary functions.
        // If the function is called using the Confkit::Config proc macro no manual input is needed. 
        pub fn generate_config_file(file_contents :String, comments :Vec<Comment>) -> Result<String, ConfigError> {
            let mut config = match DocumentMut::from_str(&file_contents) {
                Ok(val) => Ok(val),
                Err(e) => Err(ConfigError::new(
                    format!("failed to create a valid toml table from the given input: {}", e)
                )),
            }?;
                
            apply_comments(&mut config, comments)?;
            Ok(config.to_string())
        }
        
        // Iterates over passed comments and calls apply_comments_to_item for each
        pub fn apply_comments(config: &mut DocumentMut, comments: Vec<Comment>) -> Result<(), ConfigError> {
            for comment in comments {
                let item = config.get_mut(comment.key).ok_or_else(|| ConfigError::new(
                    format!("unable to find the config key '{}'", comment.key)
                ))?;
            
                apply_comments_to_item(item, &comment)?;
            }
            Ok(())
        }
    
        // Handles the application of comments to passed toml items and their children if applicable
        fn apply_comments_to_item(item: &mut Item, comment: &Comment) -> Result<(), ConfigError> {
            // Applies a comment to the input item if it has one
            if let Some(text) = comment.text {
                match item {
                    Item::Value(value) => value.decor_mut().set_suffix(format!(" # {}", text)),
                    Item::Table(table) => table.decor_mut().set_suffix(format!(" # {}", text)),
                    _ => {},
                }
            }
        
            // Recursively apply comments to all children of the passed item
            if !comment.children.is_empty() {
                let table = item.as_table_mut().ok_or_else(|| ConfigError::new(       
                     format!("config key '{}' was expected to be a table", comment.key)
                ))?;
            
                for child in &comment.children {
                    let child_item = table.get_mut(child.key).ok_or_else(|| ConfigError::new(
                        format!("unable to find the nested config key '{}'", child.key)
                    ))?;
                    
                    apply_comments_to_item(child_item, child)?;
                }
            }
            Ok(())
        }
    }
}
// This module handles the custom error used elsewhere in the crate.
pub mod error_handling {
    // Imports    
    use std::{
        error::Error,
        fmt,
    };
    
    /// An error encountered while locating, generating, or loading
    /// a configuration file.
    ///
    /// ConfigError is a string based error, error messages are intended to communicate
    /// which operation failed and why.
    #[derive(Debug)]
    pub struct ConfigError {
        message: String,
    }  
    
    #[doc(hidden)]
    impl ConfigError {
        pub fn new(content: String) -> Self {
            Self {
                message: content,
            }
        }
    }
    
    impl fmt::Display for ConfigError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let message = &self.message;
            write!(formatter, "Error: {message}")  
        }
    }
    
    impl Error for ConfigError {}
}

#[cfg(test)]
mod tests {
    use super::config_file;
    use super::private;
    
    // proc macro attribute tests
    mod attribute_tests {
       use super::test_helpers::EnvVarHelper;
       use crate::confkit::Config; 
       
       #[test]
       #[serial_test::serial]
       fn attribute_applies_comments() {
           let temp_dir = tempfile::tempdir().unwrap();
           let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            #[derive(Config)]
            struct TestStruct {
                text: String,
                #[detail(comment("This is a comment!"))]
                enabled: bool,
            }
            
            let _test_struct = TestStruct::init().unwrap();
            
            let config_path = temp_dir.path().join("teststruct/config.toml");
            let result = std::fs::read_to_string(config_path).unwrap();
            
            assert!(result.contains("enabled = false # This is a comment!"));
       }
       
       #[test]
       #[serial_test::serial]
       fn attribute_sets_defaults() {
           let temp_dir = tempfile::tempdir().unwrap();
           let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            #[derive(Config)]
            struct TestStruct {
                text: String,
                #[detail(default = true)]
                enabled: bool,
            }
            
            let _test_struct = TestStruct::init().unwrap();
            
            let config_path = temp_dir.path().join("teststruct/config.toml");
            
            let result = std::fs::read_to_string(config_path).unwrap();
            let parsed = result.parse::<toml_edit::DocumentMut>().unwrap();
            
            assert_eq!(parsed["enabled"].as_bool(), Some(true));
       }
       
       #[test]
       #[serial_test::serial]
       fn attribute_applies_nested_comments() {
           let temp_dir = tempfile::tempdir().unwrap();
           let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            #[derive(Config)]
            struct TestStruct {
                text: String,
                #[detail(nested)]
                nested: Nested,
            }
            
            #[derive(Config)]
            struct Nested {
                #[detail(comment("This is a comment!"))]
                enabled: bool,
            }
            
            let _test_struct = TestStruct::init().unwrap();
            
            let config_path = temp_dir.path().join("teststruct/config.toml");
            let result = std::fs::read_to_string(config_path).unwrap();
            
            assert!(result.contains("enabled = false # This is a comment!"));
       }
       
       #[test]
       #[serial_test::serial]
       fn attribute_renames_config_file() {
           let temp_dir = tempfile::tempdir().unwrap();
           let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            #[derive(Config)]
            #[config(file = "renamed.toml")]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let _test_struct = TestStruct::init().unwrap();
            
            let config_path = temp_dir.path().join("teststruct/renamed.toml");
            
            assert!(config_path.is_file());
       }
       
       #[test]
       #[serial_test::serial]
       fn attribute_renames_config_dir() {
           let temp_dir = tempfile::tempdir().unwrap();
           let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            #[derive(Config)]
            #[config(dir = "renamed_dir")]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let _test_struct = TestStruct::init().unwrap();
            
            let config_path = temp_dir.path().join("renamed_dir");
            
            assert!(config_path.is_dir());
       }
       
       #[test]
       #[serial_test::serial]
       fn attribute_renames_dir_and_file() {
           let temp_dir = tempfile::tempdir().unwrap();
           let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            #[derive(Config)]
            #[config(dir = "renamed_dir", file = "renamed.toml")]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let _test_struct = TestStruct::init().unwrap();
            
            let config_path = temp_dir.path().join("renamed_dir/renamed.toml");
            
            assert!(config_path.parent().unwrap().is_dir());
            assert!(config_path.is_file());
       }
    }
    
    // config_file main tests
    mod config_file_tests {
        use super::test_helpers::EnvVarHelper;
        use crate::confkit::Config;
        
        #[test]
        #[serial_test::serial]
        fn init_creates_new_config() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            #[derive(Config)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let _test_struct = TestStruct::init().unwrap();
            
            let config_path = temp_dir.path().join("teststruct/config.toml");
            
            let result = std::fs::read_to_string(config_path).unwrap();
            let parsed = result.parse::<toml_edit::DocumentMut>().unwrap();

            assert_eq!(parsed["text"].as_str(), Some(""));
            assert_eq!(parsed["enabled"].as_bool(), Some(false));
        }
        
        #[test]
        #[serial_test::serial]
        fn init_loads_existing_config() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            let config_path = temp_dir.path().join("teststruct/config.toml");
            
            let toml = r#"
        text = "Hello world!"
        enabled = true    
        "#;
            
            std::fs::create_dir(&config_path.parent().unwrap()).unwrap();
            std::fs::write(&config_path, toml).unwrap();
        
            #[derive(Config)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let test_struct = TestStruct::init().unwrap();
            
            assert_eq!(test_struct.text, "Hello world!");
            assert_eq!(test_struct.enabled, true);    
        }
        
        #[test]
        #[serial_test::serial]
        fn loads_existing_config() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            let config_path = temp_dir.path().join("teststruct/config.toml");
            
            let toml = r#"
        text = "Hello world!"
        enabled = true    
        "#;
            
            std::fs::create_dir(&config_path.parent().unwrap()).unwrap();
            std::fs::write(&config_path, toml).unwrap();
        
            #[derive(Config)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let test_struct = TestStruct::load().unwrap();
            
            assert_eq!(test_struct.text, "Hello world!");
            assert_eq!(test_struct.enabled, true);
        }
        
        #[test]
        #[serial_test::serial]
        fn correctly_errors_on_missing_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            let config_path = temp_dir.path().join("teststruct");
            
            std::fs::create_dir(&config_path).unwrap();
        
            #[derive(Config)]
            #[derive(Debug)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let test_struct = TestStruct::load();
            
            assert!(test_struct.is_err());
            
            let error_text = test_struct.unwrap_err().to_string();
            
            assert_eq!(error_text, "Error: unable to find file 'config.toml' within 'teststruct'");
        }
        
        #[test]
        #[serial_test::serial]
        fn correctly_errors_on_missing_dir() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
                    
            #[derive(Config)]
            #[derive(Debug)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }
            
            let test_struct = TestStruct::load();
            
            assert!(test_struct.is_err());
            
            let error_text = test_struct.unwrap_err().to_string();
            
            assert_eq!(error_text, "Error: unable to find directory 'teststruct' within xdg config home");
        }
        
        #[test]
        #[serial_test::serial]
        fn load_errors_on_invalid_toml() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());

            let config_path = temp_dir.path().join("teststruct/config.toml");

            std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            std::fs::write(&config_path, "This is not valid TOML.").unwrap();

            #[derive(Config)]
            #[derive(Debug)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }

            assert!(TestStruct::load().is_err());
        }
        
        #[test]
        #[serial_test::serial]
        fn load_errors_on_incorrect_field_types() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());

            let config_path = temp_dir.path().join("teststruct/config.toml");
            
            let toml = r#"
        text = "Hello world!"
        enabled = "Not a boolean"
        "#;
            
            std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            std::fs::write(&config_path, toml).unwrap();

            #[derive(Config)]
            #[derive(Debug)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }

            assert!(TestStruct::load().is_err());
        }
        
        #[test]
        #[serial_test::serial]
        fn load_errors_on_missing_keys() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());

            let config_path = temp_dir.path().join("teststruct/config.toml");
            
            let toml = r#"
        text = "Hello world!"
        "#;
            
            std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            std::fs::write(&config_path, toml).unwrap();

            #[derive(Config)]
            #[derive(Debug)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }

            assert!(TestStruct::load().is_err());
        }
        
        #[test]
        #[serial_test::serial]
        fn load_errors_on_extra_keys() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());

            let config_path = temp_dir.path().join("teststruct/config.toml");
            
            let toml = r#"
        text = "Hello world!"
        bool = false
        second_bool = false
        "#;
            
            std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            std::fs::write(&config_path, toml).unwrap();

            #[derive(Config)]
            #[derive(Debug)]
            struct TestStruct {
                text: String,
                enabled: bool,
            }

            assert!(TestStruct::load().is_err());
        }
        
    }
    
    // config_locating tests
    mod config_locating_tests {
        use super::*;
        use super::test_helpers::EnvVarHelper;
        
        #[test]
        #[serial_test::serial]
        fn finds_xdg_config_home() {
            let temp_dir = tempfile::tempdir().unwrap();
            let _env = EnvVarHelper::set("XDG_CONFIG_HOME", temp_dir.path());
            
            let result = config_file::config_locating::get_xdg_config_dir().unwrap();
            
            assert_eq!(temp_dir.path(), result.as_path());
        }
        
        #[test]
        fn finds_existing_config() {
            let temp_dir = tempfile::tempdir().unwrap();
            
            let config_path = temp_dir.path().join("test_dir/test_config.toml");
            
            std::fs::create_dir(&config_path.parent().unwrap()).unwrap();
            std::fs::File::create(&config_path).unwrap();
            
            let result = config_file::config_locating::validate_config_path(
                &config_path
            ).unwrap();
            
            assert!(result.is_none());
        }
        
        #[test]
        fn identifies_missing_config_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            
            let config_path = temp_dir.path().join("test_dir/test_config.toml");
            
            std::fs::create_dir(&config_path.parent().unwrap()).unwrap();
            
            let result = config_file::config_locating::validate_config_path(
                &config_path
            ).unwrap();
            
            assert!(matches!(result, Some(config_file::config_locating::MissingConfig::File)));
        }
        
        #[test]
        fn identifies_missing_config_dir() {
            let temp_dir = tempfile::tempdir().unwrap();
            
            let config_path = temp_dir.path().join("test_dir/test_config.toml");  
            
            let result = config_file::config_locating::validate_config_path(
                &config_path
            ).unwrap();
            
            assert!(matches!(result, Some(config_file::config_locating::MissingConfig::Directory)));  
        }
    }
    
    // config_generation tests
    mod config_generation_tests {
        use super::*;
        
        #[test]
        fn preserves_valid_toml() {
            let toml = r#"
        text = "Hello world!"
        enabled = true
        "#;
    
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), Vec::new()
            ).unwrap();
        
            let parsed = result.parse::<toml_edit::DocumentMut>().unwrap();
            assert_eq!(parsed["text"].as_str(), Some("Hello world!"));
            assert_eq!(parsed["enabled"].as_bool(), Some(true));
        }
    
        #[test]
        fn rejects_invalid_toml() {
            let toml = r#"
        This is not toml.
        "#;
    
        let result = config_file::config_generation::generate_config_file(
            toml.to_string(), Vec::new()
        );
    
        assert!(result.is_err());
        }
    
        #[test]
        fn applies_comments() {
            let toml = r#"
        text = "Hello world!"
        enabled = true    
        "#;
        
            let comments = vec![
                private::Comment {
                    key: "enabled",
                    text: Some("This is a boolean!"),
                    children: Vec::new(),
                }
            ];
        
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), comments    
            ).unwrap();
        
            assert!(result.contains("enabled = true # This is a boolean!"));
        }
    
        #[test]
        fn applies_nested_comments() {
            let toml = r#"
        [main]
        text = "Hello world!"
    
        [other]
        enabled = true    
        "#;
    
            let comments = vec![
                private::Comment {
                    key: "main",
                    text: Some("This is a section!"),
                    children: Vec::new(),
                },
                private::Comment {
                    key: "other",
                    text: None,
                    children: vec![
                        private::Comment {
                            key: "enabled",
                            text: Some("This is a nested value!"),
                            children: Vec::new(),
                        }
                    ]
                }
            ];
        
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), comments
            ).unwrap();
                
            assert!(result.contains("[main] # This is a section!"));
            assert!(result.contains("enabled = true # This is a nested value!"));
        }
        
        #[test]
        fn rejects_invalid_comment_keys() {
            let toml = r#"
        text = "Hello world!"
        enabled = true
        "#;
        
            let comments = vec![
                private::Comment {
                    key: "testing",
                    text: Some("This key does not exist"),
                    children: Vec::new()
                }
            ];
            
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), comments
            );
            
            assert!(result.is_err());
            
            let error_text = result.unwrap_err().to_string();
            
            assert_eq!(error_text, "Error: unable to find the config key 'testing'");
        }
        
        #[test]
        fn rejects_non_tables_with_nested_comments() {
            let toml = r#"
        text = "Hello world!"
        enabled = true    
        "#;
        
            let comments = vec![
                private::Comment {
                    key: "enabled",
                    text: None,
                    children: vec![
                        private::Comment {
                            key: "testing",
                            text: None,
                            children: Vec::new(),
                        }
                    ]
                }
            ];
            
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), comments
            );
            
            assert!(result.is_err());
            
            let error_text = result.unwrap_err().to_string();
            
            assert_eq!(error_text, "Error: config key 'enabled' was expected to be a table");
        }
        
        #[test]
        fn rejects_invalid_nested_comment_keys() {
            let toml = r#"
        [main]
        text = "Hello world!"
    
        [other]
        enabled = true    
        "#;
    
            let comments = vec![
                private::Comment {
                    key: "main",
                    text: None,
                    children: vec![
                        private::Comment {
                            key: "testing",
                            text: Some("This key does not exist"),
                            children: Vec::new(),
                        }
                    ],
                },
            ];
        
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), comments
            );
            
            assert!(result.is_err());
            
            let error_text = result.unwrap_err().to_string();
            
            assert_eq!(error_text, "Error: unable to find the nested config key 'testing'")      
        }
        
        #[test]
        fn allows_multi_layer_recursion() {
            let toml = r#"
        [main]
        text = "Hello world!"
        [main.nested]
        enabled = true    
        "#;
            
            let comments = vec![
                private::Comment {
                    key: "main",
                    text: None,
                    children: vec![
                        private::Comment {
                            key: "nested",
                            text: None,
                            children: vec![
                                private::Comment {
                                    key: "enabled",
                                    text: Some("This is a double nested value!"),
                                    children: Vec::new(),
                                }
                            ]
                        }
                    ]
                }
            ];
            
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), comments
            ).unwrap();
            
            assert!(result.contains("enabled = true # This is a double nested value!"));
        }
        
        #[test]
        fn preserves_toml_when_no_comments_are_provided() {
            let toml = r#"
        text = "Hello world!"
        enabled = true    
        "#;
            
            let comments = vec![
                private::Comment {
                    key: "text",
                    text: None,
                    children: Vec::new(),
                }    
            ];
        
            let result = config_file::config_generation::generate_config_file(
                toml.to_string(), comments
            ).unwrap();
            
            assert_eq!(toml, result);
        }
    }
    
    // Used by other test modules to simplify certain actions
    pub mod test_helpers {   
        // Env-var helpers
        // IMPORTANT:
        // Some of this code is marked 'unsafe', however all tests that modify system-wide environment variables
        // are run serially and restore the original value when the helper is dropped.
        
        pub struct EnvVarHelper {
            key: &'static str,
            old_value: Option<std::ffi::OsString>,
        }
        
        impl EnvVarHelper {
            pub fn set(key :&'static str, value :&std::path::Path) -> Self {
                let old_value = std::env::var_os(key);
                
                unsafe {
                    std::env::set_var(key, value);
                }
                
                Self { key, old_value }
            }
        }
        
        impl Drop for EnvVarHelper {
            fn drop(&mut self) {
                unsafe {
                    match &self.old_value {
                        Some(value) => std::env::set_var(self.key, value),
                        None => std::env::remove_var(self.key)
                    }
                }
            }
        }
    }
}
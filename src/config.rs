use crate::errors::SupersError;
use config::Config;
use globwalk::GlobWalkerBuilder;
use serde::Serialize;
use serde_derive::Deserialize;
use std::env;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::{collections::HashMap, net::IpAddr};

// Configuration management
// ========================

/// Environment varible to specify the config file.
const CONFIG_FILE_VARIABLE: &str = "SUPERS_CONF_FILE";

/// Settings file.
///
/// Default location for the local settings file. The config directory comes from the standard
/// location for configuration files for the OS.
///
/// For example, for Linux the location is `~/.config/supers/conf.yaml`.
///
const DEFAULT_CONF_FILE: &str =
    "supers/conf.{toml,yaml,yml,json,json5,ini,ron}";

/// Environment variables prefix.
///
/// This prefix gets added to the field names of `ApplicationConfig` to retrieve defaults from
/// environment variables.  The environment variables override the defaults and the
/// values from the settings file.
///
/// For example, the environment variable `SUPERS_PORT` overrides the field
/// `port` from `ApplicationConfig` defaults and from the settings file.
///
const CONFIG_VAR_PREFIX: &str = "SUPERS";

/// These are the available restart policies for programs
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum RestartPolicy {
    /// Always restart the program after it exits, regardless of exit status
    #[default]
    Always,
    /// Never restart the program, regardless of exist status
    Never,
    /// Restart the program if it exited with a non-success status, otherwise, do not restart
    OnError,
}

/// Configuration for a program to be launched and supervised by supers.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct ProgramConfig {
    /// The name of the program, used for naming the thread, logging, etc. Should be unique within a supers application
    pub name: String,
    /// The command used to start the program
    pub cmd: String,
    /// An array of arguments to the program's command.
    pub args: Vec<String>,
    /// The environment variables to set before starting the program, as key-value pairs
    pub env: HashMap<String, String>,
    /// The RestartPolicy for the program
    pub restartpolicy: RestartPolicy,
}

/// Configuration for the application iteself
#[derive(Deserialize, Serialize, Debug)]
pub struct ApplicationConfig {
    /// The name of the application
    pub app_name: String,
    /// IP Address where the web server is listening
    pub address: IpAddr,
    /// Port where the web server is listening
    pub port: u16,
    /// The programs comprising the application
    #[serde(default)]
    pub programs: Vec<ProgramConfig>,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            app_name: Default::default(),
            address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port: 8080,
            programs: Default::default(),
        }
    }
}

// Get first file matching `pattern` in `dir`, or `None` otherwise.
fn get_first_match(pattern: &str, dir: &Path) -> Option<PathBuf> {
    GlobWalkerBuilder::new(dir, pattern)
        .build()
        .ok()
        .as_mut()
        .and_then(|w| w.next())
        .and_then(|d| d.ok())
        .map(|d| d.into_path())
}

impl ApplicationConfig {
    /// Build a `ApplicationConfig` value.
    ///
    /// Read configuration from the following sources, in order:
    /// - Defaults: from the `Default` implementation for `ApplicationConfig`.
    /// - Settings file: from the file in the environment variable `SUPERS_CONF_FILE`,
    ///   or from the standard location (OS dependent) `$CONFIG/supers/conf.toml`,
    ///   if the environment variable is not set.
    /// - Settings from the environment variables prefixed with the value in the
    ///   constant `CONFIG_VAR_PREFIX`.
    ///
    pub fn from_sources() -> Result<Self, SupersError> {
        Self::from_sources_variable(
            &CONFIG_FILE_VARIABLE,
            &DEFAULT_CONF_FILE,
            &CONFIG_VAR_PREFIX,
            &dirs::config_dir().unwrap_or_default(),
        )
    }

    fn from_sources_variable(
        var: &str,
        default_config: &str,
        prefix: &str,
        config_dir: &Path,
    ) -> Result<Self, SupersError> {
        let file = if let Ok(v) = env::var(var) {
            let f = PathBuf::from(v);
            f.try_exists()?.then(|| f).ok_or_else(|| {
                SupersError::ApplicationConfigError(format!(
                    "file from variable {var} not found"
                ))
            })?
        } else {
            get_first_match(default_config, config_dir)
                .unwrap_or_else(|| "".into())
        };
        Self::from_sources_with_names(&file, prefix)
    }

    fn from_sources_with_names(
        file: &Path,
        var_prefix: &str,
    ) -> Result<Self, SupersError> {
        let file_path = file.to_str().ok_or_else(|| {
            SupersError::ApplicationConfigError(
                "path to config file cannot be converted to string".into(),
            )
        })?;
        Config::builder()
            .add_source(
                config::Config::try_from::<ApplicationConfig>(
                    &Default::default(),
                )
                .map_err(|e| {
                    SupersError::ApplicationConfigError(format!("--> {}", e))
                })?,
            )
            .add_source(config::File::with_name(file_path).required(false))
            .add_source(config::Environment::with_prefix(var_prefix))
            .build()
            .and_then(|s| s.try_deserialize::<ApplicationConfig>())
            .map_err(|e| {
                SupersError::ApplicationConfigError(format!("==> {}", e))
            })
    }
}

#[cfg(test)]
mod test {
    use super::get_first_match;
    use super::ApplicationConfig;
    use anyhow::Result;
    use std::env;
    use std::error::Error;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io::Seek;
    use std::io::Write;
    use std::{net::IpAddr, path::PathBuf, str::FromStr};
    use tempfile::TempDir;

    #[test]
    fn test_glob() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let _f1 = File::create(temp_dir.path().join("foo.a1"))?;
        let _f2 = File::create(temp_dir.path().join("foo.a2"))?;
        let _g3 = File::create(temp_dir.path().join("bar.b1"))?;

        let x = get_first_match("*.b*", temp_dir.path()).unwrap();
        assert_eq!(x.file_name(), Some(OsStr::new("bar.b1")));

        let x = get_first_match("*.a*", temp_dir.path()).unwrap();
        assert_eq!(x.file_stem(), Some(OsStr::new("foo")));

        let x = get_first_match("*.x", temp_dir.path());
        assert!(x.is_none());

        Ok(())
    }

    #[test]
    fn test_default_config() -> Result<()> {
        let x = ApplicationConfig::from_sources_variable(
            "",
            "",
            "",
            &PathBuf::from(""),
        )?;
        assert_eq!(x.port, 8080);
        assert_eq!(x.app_name, "");
        assert_eq!(x.address, IpAddr::from_str("0.0.0.0")?);
        assert!(x.programs.is_empty());
        Ok(())
    }

    fn make_test_config<F, E>(
        cfg: &ApplicationConfig,
        file_name: &str,
        serialize: F,
    ) -> Result<(TempDir, File, String)>
    where
        F: FnOnce(&ApplicationConfig) -> Result<String, E>,
        E: Error + Send + Sync + 'static,
    {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join(file_name);
        let s = serialize(cfg)?;
        let mut f = File::create(&path)?;
        f.write_all(s.as_bytes())?;
        f.rewind()?;
        Ok((temp_dir, f, path.to_string_lossy().to_string()))
    }

    #[test]
    fn test_yaml_config() -> Result<()> {
        let cfg = ApplicationConfig {
            port: 3333,
            ..Default::default()
        };
        let (_temp_dir, _p, path) =
            make_test_config(&cfg, "foo.yaml", serde_yaml::to_string)?;
        let var = uuid::Uuid::new_v4().to_string();
        env::set_var(&var, path);
        let x = ApplicationConfig::from_sources_variable(
            &var,
            "",
            "",
            &PathBuf::from(""),
        )?;
        assert_eq!(x.port, 3333);
        Ok(())
    }

    #[test]
    fn test_read_from_variable() -> Result<()> {
        let cfg = ApplicationConfig {
            port: 9999,
            ..Default::default()
        };
        let (_temp_dir, _p, path) =
            make_test_config(&cfg, "foo.toml", toml::to_string)?;
        let var = uuid::Uuid::new_v4().to_string();
        env::set_var(&var, path);
        // Should read from the file in the config variable `var`
        let x = ApplicationConfig::from_sources_variable(
            &var,
            "",
            "",
            &PathBuf::from(""),
        )?;
        assert_eq!(x.port, 9999);

        let cfg2 = ApplicationConfig {
            port: 1111,
            ..Default::default()
        };
        let (temp_dir2, _q, _path) =
            make_test_config(&cfg2, "foo.toml", toml::to_string)?;
        // Default config exists, but variable should have priority
        let y = ApplicationConfig::from_sources_variable(
            &var,
            "foo.toml",
            "",
            temp_dir2.path(),
        )?;
        assert_eq!(y.port, 9999);

        // Variable is not set, should use the default config
        let y = ApplicationConfig::from_sources_variable(
            "",
            "foo.toml",
            "",
            temp_dir2.path(),
        )?;
        assert_eq!(y.port, 1111);

        let prefix = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
        env::set_var(format!("{prefix}_PORT"), "2222");
        // Environment variable with prefix should have priority over everything
        let y = ApplicationConfig::from_sources_variable(
            &var,
            "foo.toml",
            &prefix,
            temp_dir2.path(),
        )?;
        assert_eq!(y.port, 2222);

        Ok(())
    }
}

use std::ffi::OsString;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

pub const DEFAULT_CONFIG_PATH: &str = "/etc/gpustack-rust/config.yaml";
pub const DEFAULT_DATA_DIR: &str = "/var/lib/gpustack-rust-worker";
pub const DEFAULT_LOG_DIR: &str = "/var/log/gpustack-rust-worker";
pub const DEFAULT_DOCKER_SOCKET: &str = "/var/run/docker.sock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkerConfig {
    pub server_url: Option<Url>,
    pub worker_name: Option<String>,
    pub worker_ip: Option<IpAddr>,
    pub token_file: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub model_roots: Vec<PathBuf>,
    pub docker_socket: PathBuf,
}

impl WorkerConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(server_url) = &self.server_url {
            if !matches!(server_url.scheme(), "http" | "https") {
                return Err(ConfigError::InvalidField {
                    field: "server_url",
                    reason: "must use http or https",
                });
            }
            if !server_url.username().is_empty()
                || server_url.password().is_some()
                || server_url.query().is_some()
                || server_url.fragment().is_some()
            {
                return Err(ConfigError::InvalidField {
                    field: "server_url",
                    reason: "must not contain user info, query parameters, or a fragment",
                });
            }
        }
        if self
            .worker_name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(ConfigError::InvalidField {
                field: "worker_name",
                reason: "must not be empty",
            });
        }
        validate_absolute_path("token_file", self.token_file.as_deref())?;
        validate_absolute_path("data_dir", Some(&self.data_dir))?;
        validate_absolute_path("log_dir", Some(&self.log_dir))?;
        validate_absolute_path("docker_socket", Some(&self.docker_socket))?;
        for model_root in &self.model_roots {
            validate_absolute_path("model_roots", Some(model_root))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    pub server_url: Option<String>,
    pub worker_name: Option<String>,
    pub worker_ip: Option<String>,
    pub token_file: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub log_dir: Option<PathBuf>,
    pub model_roots: Option<Vec<PathBuf>>,
    pub docker_socket: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: WorkerConfig,
    pub source: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ConfigLayer {
    server_url: Option<String>,
    worker_name: Option<String>,
    worker_ip: Option<String>,
    token_file: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    model_roots: Option<Vec<PathBuf>>,
    docker_socket: Option<PathBuf>,
}

impl ConfigLayer {
    fn merge(&mut self, other: Self) {
        if other.server_url.is_some() {
            self.server_url = other.server_url;
        }
        if other.worker_name.is_some() {
            self.worker_name = other.worker_name;
        }
        if other.worker_ip.is_some() {
            self.worker_ip = other.worker_ip;
        }
        if other.token_file.is_some() {
            self.token_file = other.token_file;
        }
        if other.data_dir.is_some() {
            self.data_dir = other.data_dir;
        }
        if other.log_dir.is_some() {
            self.log_dir = other.log_dir;
        }
        if other.model_roots.is_some() {
            self.model_roots = other.model_roots;
        }
        if other.docker_socket.is_some() {
            self.docker_socket = other.docker_socket;
        }
    }

    fn from_environment<I, K, V>(variables: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        let mut layer = Self::default();
        for (key, value) in variables {
            let key = key.into().to_string_lossy().into_owned();
            let value = value.into().to_string_lossy().into_owned();
            match key.as_str() {
                "GPUSTACK_RS_SERVER_URL" => layer.server_url = Some(value),
                "GPUSTACK_RS_WORKER_NAME" => layer.worker_name = Some(value),
                "GPUSTACK_RS_WORKER_IP" => layer.worker_ip = Some(value),
                "GPUSTACK_RS_TOKEN_FILE" => layer.token_file = Some(PathBuf::from(value)),
                "GPUSTACK_RS_DATA_DIR" => layer.data_dir = Some(PathBuf::from(value)),
                "GPUSTACK_RS_LOG_DIR" => layer.log_dir = Some(PathBuf::from(value)),
                "GPUSTACK_RS_MODEL_ROOTS" => {
                    let roots = value
                        .split(',')
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                        .map(PathBuf::from)
                        .collect::<Vec<_>>();
                    if roots.is_empty() {
                        return Err(ConfigError::InvalidEnvironment { key });
                    }
                    layer.model_roots = Some(roots);
                }
                "GPUSTACK_RS_DOCKER_SOCKET" => {
                    layer.docker_socket = Some(PathBuf::from(value));
                }
                _ => {}
            }
        }
        Ok(layer)
    }
}

impl From<ConfigOverrides> for ConfigLayer {
    fn from(value: ConfigOverrides) -> Self {
        Self {
            server_url: value.server_url,
            worker_name: value.worker_name,
            worker_ip: value.worker_ip,
            token_file: value.token_file,
            data_dir: value.data_dir,
            log_dir: value.log_dir,
            model_roots: value.model_roots,
            docker_socket: value.docker_socket,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read configuration file {path}: {kind:?}")]
    Read {
        path: PathBuf,
        kind: std::io::ErrorKind,
    },
    #[error("failed to parse configuration file {path} at line {line}, column {column}")]
    Parse {
        path: PathBuf,
        line: usize,
        column: usize,
    },
    #[error("invalid environment variable {key}")]
    InvalidEnvironment { key: String },
    #[error("invalid configuration field {field}: {reason}")]
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },
}

pub fn load_config(
    config_path: Option<&Path>,
    overrides: ConfigOverrides,
) -> Result<LoadedConfig, ConfigError> {
    load_config_with_environment(config_path, std::env::vars_os(), overrides)
}

fn load_config_with_environment<I, K, V>(
    config_path: Option<&Path>,
    environment: I,
    overrides: ConfigOverrides,
) -> Result<LoadedConfig, ConfigError>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<OsString>,
    V: Into<OsString>,
{
    let explicit_path = config_path.is_some();
    let selected_path = config_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

    let (mut layer, source) = if selected_path.exists() || explicit_path {
        let contents = fs::read_to_string(&selected_path).map_err(|error| ConfigError::Read {
            path: selected_path.clone(),
            kind: error.kind(),
        })?;
        let parsed = serde_saphyr::from_str::<ConfigLayer>(&contents).map_err(|error| {
            let location = error.location();
            ConfigError::Parse {
                path: selected_path.clone(),
                line: location.map_or(0, |location| location.line() as usize),
                column: location.map_or(0, |location| location.column() as usize),
            }
        })?;
        (parsed, Some(selected_path))
    } else {
        (ConfigLayer::default(), None)
    };

    layer.merge(ConfigLayer::from_environment(environment)?);
    layer.merge(overrides.into());

    let config = build_config(layer)?;
    config.validate()?;
    Ok(LoadedConfig { config, source })
}

fn build_config(layer: ConfigLayer) -> Result<WorkerConfig, ConfigError> {
    let server_url = layer
        .server_url
        .map(|value| {
            Url::parse(&value).map_err(|_| ConfigError::InvalidField {
                field: "server_url",
                reason: "must be a valid absolute URL",
            })
        })
        .transpose()?;
    let worker_ip = layer
        .worker_ip
        .map(|value| {
            value
                .parse::<IpAddr>()
                .map_err(|_| ConfigError::InvalidField {
                    field: "worker_ip",
                    reason: "must be a valid IP address",
                })
        })
        .transpose()?;

    Ok(WorkerConfig {
        server_url,
        worker_name: layer.worker_name,
        worker_ip,
        token_file: layer.token_file,
        data_dir: layer
            .data_dir
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR)),
        log_dir: layer
            .log_dir
            .unwrap_or_else(|| PathBuf::from(DEFAULT_LOG_DIR)),
        model_roots: layer.model_roots.unwrap_or_default(),
        docker_socket: layer
            .docker_socket
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DOCKER_SOCKET)),
    })
}

fn validate_absolute_path(field: &'static str, path: Option<&Path>) -> Result<(), ConfigError> {
    if path.is_some_and(|path| !path.is_absolute()) {
        return Err(ConfigError::InvalidField {
            field,
            reason: "must be an absolute path",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_is_cli_then_environment_then_yaml_then_defaults() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let config_path = directory.path().join("worker.yaml");
        fs::write(
            &config_path,
            "worker_name: yaml-worker\ndata_dir: /yaml/data\nlog_dir: /yaml/log\n",
        )
        .expect("write configuration");
        let environment = [
            ("GPUSTACK_RS_WORKER_NAME", "env-worker"),
            ("GPUSTACK_RS_DATA_DIR", "/env/data"),
        ];
        let overrides = ConfigOverrides {
            worker_name: Some("cli-worker".to_owned()),
            ..ConfigOverrides::default()
        };

        let loaded = load_config_with_environment(Some(&config_path), environment, overrides)
            .expect("load configuration");

        assert_eq!(loaded.config.worker_name.as_deref(), Some("cli-worker"));
        assert_eq!(loaded.config.data_dir, PathBuf::from("/env/data"));
        assert_eq!(loaded.config.log_dir, PathBuf::from("/yaml/log"));
        assert_eq!(
            loaded.config.docker_socket,
            PathBuf::from(DEFAULT_DOCKER_SOCKET)
        );
    }

    #[test]
    fn rejected_inline_secret_is_not_echoed() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let config_path = directory.path().join("worker.yaml");
        let secret = "do-not-print-this-token";
        fs::write(&config_path, format!("token: {secret}\n")).expect("write configuration");

        let error = load_config_with_environment(
            Some(&config_path),
            std::iter::empty::<(String, String)>(),
            ConfigOverrides::default(),
        )
        .expect_err("inline token must be rejected");

        assert!(!error.to_string().contains(secret));
    }
}

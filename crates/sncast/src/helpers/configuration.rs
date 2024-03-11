use crate::ValidatedWaitParams;
use anyhow::Result;
use camino::Utf8PathBuf;
use configuration::{get_property, GlobalConfig};
use serde::{Deserialize, Serialize};
use toml::Value;

use super::constants::{WAIT_RETRY_INTERVAL, WAIT_TIMEOUT};

#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct CastConfig {
    pub rpc_url: String,
    pub account: String,
    pub accounts_file: Utf8PathBuf,
    pub keystore: Option<Utf8PathBuf>,
    pub wait_params: ValidatedWaitParams,
}

impl GlobalConfig for CastConfig {
    fn tool_name() -> String {
        String::from("sncast")
    }

    fn from_raw(config: &Value) -> Result<Self> {
        Ok(CastConfig {
            rpc_url: get_property(config, "url").unwrap_or(String::default()),
            account: get_property(config, "account").unwrap_or(String::default()),
            accounts_file: get_property(config, "accounts-file").unwrap_or(Utf8PathBuf::default()),
            keystore: get_property(config, "keystore"),
            wait_params: ValidatedWaitParams::new(
                get_property(config, "wait-retry-interval").unwrap_or(WAIT_RETRY_INTERVAL),
                get_property(config, "wait-timeout").unwrap_or(WAIT_TIMEOUT),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = CastConfig::default();
        assert_eq!(config.wait_params.get_timeout(), WAIT_TIMEOUT);
        assert_eq!(config.wait_params.get_retry_interval(), WAIT_RETRY_INTERVAL);
    }
}

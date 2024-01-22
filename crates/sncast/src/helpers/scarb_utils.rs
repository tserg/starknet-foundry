use super::constants::{WAIT_RETRY_INTERVAL, WAIT_TIMEOUT};
use anyhow::{anyhow, bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use scarb_api::{get_contracts_map, ScarbCommand, StarknetContractArtifacts};
use scarb_metadata;
use scarb_metadata::{Metadata, PackageMetadata};
use scarb_ui::args::PackagesFilter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::default::Default;
use std::env;
use std::str::FromStr;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CastConfig {
    pub rpc_url: String,
    pub account: String,
    pub accounts_file: Utf8PathBuf,
    pub keystore: Option<Utf8PathBuf>,
    pub wait_timeout: u16,
    pub wait_retry_interval: u8,
}

impl CastConfig {
    pub fn from_package_tool_sncast(
        package_tool_sncast: &Value,
        profile: &Option<String>,
    ) -> Result<CastConfig> {
        let tool = get_profile(package_tool_sncast, profile)?;

        Ok(CastConfig {
            rpc_url: get_property(tool, "url"),
            account: get_property(tool, "account"),
            accounts_file: get_property(tool, "accounts-file"),
            keystore: get_property_optional(tool, "keystore"),
            wait_timeout: get_property(tool, "wait-timeout"),
            wait_retry_interval: get_property(tool, "wait-retry-interval"),
        })
    }
}

impl Default for CastConfig {
    fn default() -> Self {
        Self {
            rpc_url: String::default(),
            account: String::default(),
            accounts_file: Utf8PathBuf::default(),
            keystore: None,
            wait_timeout: WAIT_TIMEOUT,
            wait_retry_interval: WAIT_RETRY_INTERVAL,
        }
    }
}

pub struct BuildConfig {
    pub scarb_toml_path: Utf8PathBuf,
    pub json: bool,
}

pub trait PropertyFromCastConfig: Sized {
    fn from_toml_value(value: &Value) -> Option<Self>;
    fn default_value() -> Self;
}

impl PropertyFromCastConfig for String {
    fn from_toml_value(value: &Value) -> Option<Self> {
        value.as_str().map(std::borrow::ToOwned::to_owned)
    }

    fn default_value() -> Self {
        String::default()
    }
}

impl PropertyFromCastConfig for Utf8PathBuf {
    fn from_toml_value(value: &Value) -> Option<Self> {
        value.as_str().map(Utf8PathBuf::from)
    }

    fn default_value() -> Self {
        Utf8PathBuf::default()
    }
}

impl PropertyFromCastConfig for u8 {
    fn from_toml_value(value: &Value) -> Option<Self> {
        value.as_u64().and_then(|i| i.try_into().ok())
    }

    fn default_value() -> Self {
        WAIT_RETRY_INTERVAL
    }
}

impl PropertyFromCastConfig for u16 {
    fn from_toml_value(value: &Value) -> Option<Self> {
        value.as_u64().and_then(|i| i.try_into().ok())
    }

    fn default_value() -> Self {
        WAIT_TIMEOUT
    }
}

impl<T> PropertyFromCastConfig for Option<T>
where
    T: PropertyFromCastConfig,
{
    fn from_toml_value(value: &Value) -> Option<Self> {
        T::from_toml_value(value).map(Some)
    }
    fn default_value() -> Self {
        Some(T::default_value())
    }
}

pub fn get_profile<'a>(tool_sncast: &'a Value, profile: &Option<String>) -> Result<&'a Value> {
    match profile {
        Some(profile_) => tool_sncast
            .get(profile_)
            .ok_or_else(|| anyhow!("No field [tool.sncast.{}] found in package", profile_)),
        None => Ok(tool_sncast),
    }
}

pub fn get_property<T>(tool: &Value, field: &str) -> T
where
    T: PropertyFromCastConfig + Default,
{
    get_property_optional(tool, field).unwrap_or_else(T::default_value)
}

pub fn get_property_optional<T>(tool: &Value, field: &str) -> Option<T>
where
    T: PropertyFromCastConfig + Default,
{
    tool.get(field).and_then(T::from_toml_value)
}

pub fn get_scarb_manifest() -> Result<Utf8PathBuf> {
    get_scarb_manifest_for(<&Utf8Path>::from("."))
}

pub fn get_scarb_manifest_for(dir: &Utf8Path) -> Result<Utf8PathBuf> {
    ScarbCommand::new().ensure_available()?;

    let output = ScarbCommand::new()
        .current_dir(dir)
        .arg("manifest-path")
        .command()
        .output()
        .context("Failed to execute the `scarb manifest-path` command")?;

    let output_str = String::from_utf8(output.stdout)
        .context("`scarb manifest-path` command failed to provide valid output")?;

    let path = Utf8PathBuf::from_str(output_str.trim())
        .context("`scarb manifest-path` failed. Invalid location returned")?;

    Ok(path)
}

fn get_scarb_metadata_command(
    manifest_path: &Utf8PathBuf,
) -> Result<scarb_metadata::MetadataCommand> {
    ScarbCommand::new().ensure_available()?;

    let mut command = scarb_metadata::MetadataCommand::new();
    command.inherit_stderr().manifest_path(manifest_path);
    Ok(command)
}

fn execute_scarb_metadata_command(
    command: &scarb_metadata::MetadataCommand,
) -> Result<scarb_metadata::Metadata> {
    command.exec().context(format!(
        "Failed to read the `Scarb.toml` manifest file. Doesn't exist in the current or parent directories = {}",
        env::current_dir()
            .expect("Failed to access the current directory")
            .into_os_string()
            .into_string()
            .expect("Failed to convert current directory into a string")
    ))
}

pub fn get_scarb_metadata(manifest_path: &Utf8PathBuf) -> Result<scarb_metadata::Metadata> {
    let mut command = get_scarb_metadata_command(manifest_path)?;
    let command = command.no_deps();
    execute_scarb_metadata_command(command)
}

pub fn get_scarb_metadata_with_deps(
    manifest_path: &Utf8PathBuf,
) -> Result<scarb_metadata::Metadata> {
    let command = get_scarb_metadata_command(manifest_path)?;
    execute_scarb_metadata_command(&command)
}

pub fn ensure_scarb_manifest_path(path_to_scarb_toml: &Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    if let Some(path) = path_to_scarb_toml {
        assert!(path.exists(), "Failed to locate file at path = {path}");
    }

    let manifest_path = path_to_scarb_toml.clone().unwrap_or_else(|| {
        get_scarb_manifest().expect("Failed to obtain manifest path from scarb")
    });

    if !manifest_path.exists() {
        return Err(anyhow!(
            "Path to Scarb.toml manifest does not exist = {manifest_path}"
        ));
    }

    Ok(manifest_path)
}

fn get_package_metadata_by_name<'a>(
    metadata: &'a Metadata,
    package_name: &str,
) -> Result<&'a PackageMetadata> {
    metadata
        .packages
        .iter()
        .find(|package| package.name == package_name)
        .ok_or(anyhow!(
            "Package {} not found in scarb metadata",
            &package_name
        ))
}

fn get_default_package_metadata(metadata: &Metadata) -> Result<&PackageMetadata> {
    match metadata.packages.iter().collect::<Vec<_>>().as_slice() {
        [package] => Ok(package),
        [] => Err(anyhow!("No package found in metadata")),
        _ => Err(anyhow!(
            "More than one package found in metadata - specify package using --package flag"
        )),
    }
}

pub fn get_package_metadata(
    manifest_path: &Utf8PathBuf,
    package_name: &Option<String>,
) -> Result<PackageMetadata> {
    let metadata = get_scarb_metadata(manifest_path)?;
    match &package_name {
        Some(package_name) => Ok(get_package_metadata_by_name(&metadata, package_name)?.clone()),
        None => Ok(get_default_package_metadata(&metadata)?.clone()),
    }
}

pub fn parse_scarb_config(
    profile: &Option<String>,
    path: &Option<Utf8PathBuf>,
    package_name: &Option<String>,
) -> Result<CastConfig> {
    let manifest_path = match path.clone() {
        Some(path) => {
            if !(path.exists()) {
                bail!("Failed to locate file at path = {path}");
            }
            path
        }
        None => get_scarb_manifest().context("Failed to obtain manifest path from scarb")?,
    };

    if !manifest_path.exists() {
        return Ok(CastConfig::default());
    }

    let metadata = get_package_metadata(&manifest_path, package_name)
        .expect("Failed to fetch package metadata");

    match get_package_tool_sncast(&metadata) {
        Ok(package_tool_sncast) => {
            CastConfig::from_package_tool_sncast(package_tool_sncast, profile)
        }
        Err(_) => Ok(CastConfig::default()),
    }
}

pub fn get_package_tool_sncast(metadata: &PackageMetadata) -> Result<&Value> {
    let tool = metadata
        .manifest_metadata
        .tool
        .as_ref()
        .ok_or_else(|| anyhow!("No field [tool] found in package"))?;

    let tool_sncast = tool
        .get("sncast")
        .ok_or_else(|| anyhow!("No field [tool.sncast] found in package"))?;

    Ok(tool_sncast)
}

pub fn build(
    package: &PackageMetadata,
    config: &BuildConfig,
) -> Result<HashMap<String, StarknetContractArtifacts>> {
    let filter = PackagesFilter::generate_for::<Metadata>([package].into_iter());

    let mut cmd = ScarbCommand::new_with_stdio();
    cmd.arg("build")
        .manifest_path(&config.scarb_toml_path)
        .packages_filter(filter);
    if config.json {
        cmd.json();
    }
    cmd.run()
        .map_err(|e| anyhow!(format!("Failed to build using scarb; {e}")))?;

    let metadata = get_scarb_metadata_with_deps(&config.scarb_toml_path)?;
    get_contracts_map(&metadata, &package.id)
}

#[cfg(test)]
mod tests {
    use crate::helpers::scarb_utils::parse_scarb_config;
    use crate::helpers::scarb_utils::CastConfig;
    use crate::helpers::scarb_utils::{get_package_metadata, get_scarb_metadata};
    use crate::helpers::scarb_utils::{WAIT_RETRY_INTERVAL, WAIT_TIMEOUT};
    use camino::Utf8PathBuf;
    use sealed_test::prelude::rusty_fork_test;
    use sealed_test::prelude::sealed_test;

    #[test]
    fn test_parse_scarb_config_happy_case_with_profile() {
        let config = parse_scarb_config(
            &Some(String::from("myprofile")),
            &Some(Utf8PathBuf::from(
                "tests/data/contracts/constructor_with_params/Scarb.toml",
            )),
            &None,
        )
        .unwrap();

        assert_eq!(config.account, String::from("user1"));
        assert_eq!(config.rpc_url, String::from("http://127.0.0.1:5055/rpc"));
    }

    #[test]
    fn test_parse_scarb_config_happy_case_without_profile() {
        let config = parse_scarb_config(
            &None,
            &Some(Utf8PathBuf::from("tests/data/contracts/map/Scarb.toml")),
            &Some("map".to_string()),
        )
        .unwrap();
        assert_eq!(config.account, String::from("user2"));
        assert_eq!(config.rpc_url, String::from("http://127.0.0.1:5055/rpc"));
    }

    #[test]
    fn test_parse_scarb_config_not_found() {
        let config = parse_scarb_config(
            &None,
            &Some(Utf8PathBuf::from("whatever/Scarb.toml")),
            &None,
        )
        .unwrap_err();
        assert!(config
            .to_string()
            .contains("Failed to locate file at path = whatever/Scarb.toml"));
    }

    #[test]
    fn test_parse_scarb_config_no_path_not_found() {
        let config = parse_scarb_config(&None, &None, &None).unwrap();

        assert!(config.rpc_url.is_empty());
        assert!(config.account.is_empty());
    }

    #[test]
    fn test_parse_scarb_config_not_in_file() {
        let config = parse_scarb_config(
            &None,
            &Some(Utf8PathBuf::from("tests/data/files/noconfig_Scarb.toml")),
            &None,
        )
        .unwrap();

        assert!(config.rpc_url.is_empty());
        assert!(config.account.is_empty());
    }

    #[test]
    fn test_parse_scarb_config_no_profile_found() {
        let config = parse_scarb_config(
            &Some(String::from("mariusz")),
            &Some(Utf8PathBuf::from("tests/data/contracts/map/Scarb.toml")),
            &None,
        )
        .unwrap_err();
        assert_eq!(
            config.to_string(),
            "No field [tool.sncast.mariusz] found in package"
        );
    }

    #[test]
    fn test_parse_scarb_config_account_missing() {
        let config = parse_scarb_config(
            &None,
            &Some(Utf8PathBuf::from("tests/data/files/somemissing_Scarb.toml")),
            &None,
        )
        .unwrap();

        assert!(config.account.is_empty());
    }

    #[sealed_test(files = ["tests/data/contracts/no_sierra/Scarb.toml"])]
    fn test_parse_scarb_config_no_profile_no_path() {
        let config = parse_scarb_config(&None, &None, &None).unwrap();

        assert!(config.rpc_url.is_empty());
        assert!(config.account.is_empty());
    }

    #[sealed_test(files = ["tests/data/contracts/constructor_with_params/Scarb.toml"])]
    fn test_parse_scarb_config_no_path() {
        let config = parse_scarb_config(&Some(String::from("myprofile")), &None, &None).unwrap();

        assert_eq!(config.rpc_url, String::from("http://127.0.0.1:5055/rpc"));
        assert_eq!(config.account, String::from("user1"));
    }

    #[test]
    fn test_get_scarb_metadata() {
        let metadata = get_scarb_metadata(&"tests/data/contracts/map/Scarb.toml".into());
        assert!(metadata.is_ok());
    }

    #[test]
    fn test_get_scarb_metadata_not_found() {
        let metadata_err = get_scarb_metadata(&"Scarb.toml".into()).unwrap_err();
        assert!(metadata_err
            .to_string()
            .contains("Failed to read the `Scarb.toml` manifest file."));
    }

    #[test]
    fn test_config_defaults() {
        let config = CastConfig::default();
        assert_eq!(config.wait_timeout, WAIT_TIMEOUT);
        assert_eq!(config.wait_retry_interval, WAIT_RETRY_INTERVAL);
    }

    #[test]
    fn test_get_package_metadata_happy_default() {
        let metadata =
            get_package_metadata(&"tests/data/contracts/map/Scarb.toml".into(), &None).unwrap();
        assert_eq!(metadata.name, "map");
    }

    #[test]
    fn test_get_package_metadata_happy_by_name() {
        let metadata = get_package_metadata(
            &"tests/data/contracts/multiple_packages/Scarb.toml".into(),
            &Some("package2".into()),
        )
        .unwrap();
        assert_eq!(metadata.name, "package2");
    }

    #[test]
    #[should_panic(
        expected = "More than one package found in metadata - specify package using --package flag"
    )]
    fn test_get_package_metadata_more_than_one_default() {
        get_package_metadata(
            &"tests/data/contracts/multiple_packages/Scarb.toml".into(),
            &None,
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Package whatever not found in scarb metadata")]
    fn test_get_package_metadata_no_such_package() {
        let metadata = get_package_metadata(
            &"tests/data/contracts/multiple_packages/Scarb.toml".into(),
            &Some("whatever".into()),
        )
        .unwrap();
        assert_eq!(metadata.name, "package2");
    }
}

pub static DEFAULT_MULTICALL_CONTENTS: &str = r#"[[call]]
call_type = "deploy"
class_hash = ""
inputs = []
id = ""
unique = false

[[call]]
call_type = "invoke"
contract_address = ""
function = ""
inputs = []
"#;

pub const UDC_ADDRESS: &str = "0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf";
pub const OZ_CLASS_HASH: &str = "0x4c6d6cf894f8bc96bb9c525e6853e5483177841f7388f74a46cfda6f028c755";

// used in wait_for_tx. Txs will be fetched every 5s with timeout of 300s - so 60 attempts
#[allow(dead_code)]
pub const WAIT_TIMEOUT: u16 = 300;
#[allow(dead_code)]
pub const WAIT_RETRY_INTERVAL: u8 = 5;

#[allow(dead_code)]
pub const DEFAULT_ACCOUNTS_FILE: &str = "~/.starknet_accounts/starknet_open_zeppelin_accounts.json";

pub const KEYSTORE_PASSWORD_ENV_VAR: &str = "KEYSTORE_PASSWORD";
pub const CREATE_KEYSTORE_PASSWORD_ENV_VAR: &str = "CREATE_KEYSTORE_PASSWORD";

pub const SCRIPT_LIB_ARTIFACT_NAME: &str = "__sncast_script_lib";

pub const STATE_FILE_VERSION: u8 = 1;

pub const INIT_SCRIPTS_DIR: &str = "scripts";

pub const DEFAULT_STATE_FILE_NAME: &str = "state.json";

use crate::helpers::constants::{SCRIPTS_DIR, URL};
use indoc::indoc;
use snapbox::cmd::{cargo_bin, Command};

#[tokio::test]
async fn test_happy_case() {
    let script_name = "map_script";
    let args = vec![
        "--accounts-file",
        "../../../accounts/accounts.json",
        "--account",
        "user4",
        "--url",
        URL,
        "script",
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR.to_owned() + "/map_script/scripts")
        .args(args);

    snapbox.assert().success().stdout_matches(indoc! {r"
        ...
        command: script
        status: success
    "});
}

#[tokio::test]
async fn test_run_script_from_different_directory() {
    let script_name = "call_happy";
    let path_to_scarb_toml = "misc/Scarb.toml";
    let args = vec![
        "--accounts-file",
        "../accounts/accounts.json",
        "--account",
        "user1",
        "--url",
        URL,
        "--path-to-scarb-toml",
        path_to_scarb_toml,
        "script",
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR)
        .args(args);
    snapbox.assert().success().stdout_matches(indoc! {r"
        ...
        command: script
        status: success
    "});
}

#[tokio::test]
async fn test_run_script_from_different_directory_no_path_to_scarb_toml() {
    let script_name = "call_happy";
    let args = vec![
        "--accounts-file",
        "../accounts/accounts.json",
        "--account",
        "user1",
        "--url",
        URL,
        "script",
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR)
        .args(args);
    snapbox.assert().failure().stderr_matches(indoc! {r"
        Error: Path to Scarb.toml manifest does not exist =[..]
    "});
}

#[tokio::test]
async fn test_fail_when_using_starknet_syscall() {
    let script_name = "using_starknet_syscall";
    let args = vec![
        "--accounts-file",
        "../../accounts/accounts.json",
        "--account",
        "user1",
        "--url",
        URL,
        "script",
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR.to_owned() + "/misc")
        .args(args);
    snapbox.assert().success().stderr_matches(indoc! {r"
        ...
        command: script
        error: Got an exception while executing a hint: Hint Error: Starknet syscalls are not supported
    "});
}

#[tokio::test]
async fn test_incompatible_sncast_std_version() {
    let script_name = "map_script";
    let args = vec![
        "--accounts-file",
        "../../../accounts/accounts.json",
        "--account",
        "user4",
        "--url",
        URL,
        "script",
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR.to_owned() + "/old_sncast_std/scripts")
        .args(args);

    snapbox.assert().success().stdout_matches(indoc! {r"
        ...
        Warning: Package sncast_std version does not meet the recommended version requirement =0.16.0, it might result in unexpected behaviour
        ...
    "});
}

#[tokio::test]
async fn test_multiple_packages_not_picked() {
    let script_name = "script1";
    let args = vec![
        "--accounts-file",
        "../../accounts/accounts.json",
        "--account",
        "user4",
        "--url",
        URL,
        "script",
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR.to_owned() + "/packages")
        .args(args);

    snapbox.assert().failure().stderr_matches(indoc! {r"
        ...
        Error: More than one package found in scarb metadata - specify package using --package flag
    "});
}

#[tokio::test]
async fn test_multiple_packages_happy_case() {
    let script_name = "script1";
    let args = vec![
        "--accounts-file",
        "../../accounts/accounts.json",
        "--account",
        "user4",
        "--url",
        URL,
        "script",
        "--package",
        &script_name,
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR.to_owned() + "/packages")
        .args(args);

    snapbox.assert().success().stdout_matches(indoc! {r"
        ...
        command: script
        status: success
    "});
}

#[tokio::test]
async fn test_run_script_display_debug_traits() {
    let script_name = "display_debug_traits_for_subcommand_responses";
    let args = vec![
        "--accounts-file",
        "../../../accounts/accounts.json",
        "--account",
        "user4",
        "--url",
        URL,
        "script",
        &script_name,
    ];

    let snapbox = Command::new(cargo_bin!("sncast"))
        .current_dir(SCRIPTS_DIR.to_owned() + "/map_script/scripts")
        .args(args);

    snapbox.assert().success().stdout_matches(indoc! {r"
        ...
        declare_nonce: 1
        debug declare_nonce: 1
        Transaction hash = 0x[..]
        declare_result: class_hash: [..], transaction_hash: [..]
        debug declare_result: DeclareResult { class_hash: [..], transaction_hash: [..] }
        Transaction hash = 0x[..]
        deploy_result: contract_address: [..], transaction_hash: [..]
        debug deploy_result: DeployResult { contract_address: [..], transaction_hash: [..] }
        Transaction hash = 0x[..]
        invoke_result: [..]
        debug invoke_result: InvokeResult { transaction_hash: [..] }
        call_result: [2]
        debug call_result: CallResult { data: [2] }
        command: script
        status: success
    "});
}

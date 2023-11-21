# Cairo deployment scripts

## Overview

Cast allows for writing Cairo scripts, designed to interact with Starknet. Currently, the available commands include `call`, `invoke`, `declare` and `deploy`.
The commands can be imported from the `sncast_std` package. The code is available in Starknet Foundry main repo (https://github.com/foundry-rs/starknet-foundry).
As in the normal Scarb package, the scripts has to be placed in the `src` directory.

## Examples

### Calling/invoking contract in deployment script
Scarb.toml
```toml
[package]
name = "hello_world_script"
version = "0.1.0"

[dependencies]
starknet = ">=2.3.0"
sncast_std = { path = "../../../../../../sncast_std" }
```

my_script_name.cairo
```rust
use sncast_std::{call, CallResult};

fn main() {
    let eth = 0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7;
    let addr = 0x0089496091c660345BaA480dF76c1A900e57cf34759A899eFd1EADb362b20DB5;
    let call_result = call(eth.try_into().unwrap(), 'allowance', array![addr, addr]);
    let call_result = *call_result.data[0];
    assert(call_result == 0, call_result);

    let call_result = call(eth.try_into().unwrap(), 'decimals', array![]);
    let call_result = *call_result.data[0];
    assert(call_result == 18, call_result);
}
```

Launching the script from the folder that contains `Scarb.toml`
```shell
$ sncast --account myuser \
    --url http://127.0.0.1:5050/rpc \ 
    script my_script_name
```

### Declaring contract in deployment script

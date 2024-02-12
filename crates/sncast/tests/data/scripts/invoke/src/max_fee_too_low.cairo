use sncast_std::{invoke, InvokeResult, ScriptCommandError, ProviderError, StarknetError};
use starknet::{ContractAddress, Felt252TryIntoContractAddress};
use traits::Into;

fn main() {
    println!("test");
    let map_contract_address = 0x059e877cd42aec5604601f81b5eabd346fc9b0fbbbfba3253859cb68e1d52614
        .try_into()
        .expect('Invalid contract address value');

    let invoke_result = invoke(
        map_contract_address, 'put', array![0x10, 0x1], Option::Some(1), Option::None
    )
        .unwrap_err();
    println!("{:?}", invoke_result);
}


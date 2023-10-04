use crate::state::ExtendedStateReader;
use blockifier::state::cached_state::{CachedState, GlobalContractCache};
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::ContractAddressSalt;
use state::CheatcodeState;

pub mod cheatcodes;
pub mod constants;
pub mod execution;
pub mod forking;
pub mod panic_data;
pub mod rpc;
pub mod state;

pub struct CheatnetState {
    cheatcode_state: CheatcodeState,
    pub blockifier_state: CachedState<ExtendedStateReader>,
    pub deploy_salt_base: u32,
    pub available_steps: Option<u32>,
}

impl CheatnetState {
    #[must_use]
    pub fn new(state: ExtendedStateReader, max_steps: Option<u32>) -> Self {
        CheatnetState {
            cheatcode_state: CheatcodeState::new(),
            blockifier_state: CachedState::new(state, GlobalContractCache::default()),
            deploy_salt_base: 0,
            available_steps: max_steps,
        }
    }

    pub fn increment_deploy_salt_base(&mut self) {
        self.deploy_salt_base += 1;
    }

    #[must_use]
    pub fn get_salt(&self) -> ContractAddressSalt {
        ContractAddressSalt(StarkFelt::from(self.deploy_salt_base))
    }

    pub fn decrease_available_steps(&mut self, used_steps: u32) {
        self.available_steps = Some(self.available_steps.unwrap() - used_steps);
    }
}

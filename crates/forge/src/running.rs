use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use blockifier::execution::cairo1_execution::{
    finalize_execution, prepare_call_arguments, VmExecutionContext,
};
use blockifier::execution::contract_class::EntryPointV1;
use blockifier::execution::entry_point::{
    CallEntryPoint, CallType, EntryPointExecutionContext, ExecutionResources,
};
use blockifier::execution::errors::PreExecutionError;
use blockifier::execution::execution_utils::{
    write_maybe_relocatable, write_stark_felt, ReadOnlySegments,
};
use blockifier::execution::syscalls::hint_processor::SyscallHintProcessor;
use blockifier::state::cached_state::{CachedState, GlobalContractCache};
use blockifier::state::state_api::State;
use cairo_felt::Felt252;
use cairo_vm::serde::deserialize_program::{BuiltinName, HintParams, ReferenceManager};
use cairo_vm::types::relocatable::{MaybeRelocatable, Relocatable};
use cheatnet::constants::{build_block_context, build_testing_state, build_transaction_context};
use cheatnet::CheatnetState;
use itertools::chain;

use cairo_lang_casm::hints::Hint;
use cairo_lang_casm::instructions::Instruction;
use cairo_lang_runner::casm_run::hint_to_hint_params;
use cairo_lang_runner::RunnerError;
use cairo_lang_runner::SierraCasmRunner;
use cairo_vm::types::program::Program;
use cairo_vm::vm::runners::cairo_runner::{CairoRunner, RunResources};
use cairo_vm::vm::vm_core::VirtualMachine;
use camino::Utf8PathBuf;
use cheatnet::execution::cairo1_execution::cheatable_run_entry_point;
use cheatnet::state::ExtendedStateReader;
use starknet::core::utils::get_selector_from_name;
use starknet_api::core::PatriciaKey;
use starknet_api::core::{ContractAddress, EntryPointSelector};
use starknet_api::deprecated_contract_class::{EntryPointOffset, EntryPointType};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::transaction::Calldata;
use starknet_api::{patricia_key, stark_felt};
use test_collector::TestCase;

use crate::cheatcodes_hint_processor::CairoHintProcessor;
use crate::scarb::StarknetContractArtifacts;
use crate::test_case_summary::TestCaseSummary;

/// Builds `hints_dict` required in `cairo_vm::types::program::Program` from instructions.
fn build_hints_dict<'b>(
    instructions: impl Iterator<Item = &'b Instruction>,
) -> (HashMap<usize, Vec<HintParams>>, HashMap<String, Hint>) {
    let mut hints_dict: HashMap<usize, Vec<HintParams>> = HashMap::new();
    let mut string_to_hint: HashMap<String, Hint> = HashMap::new();

    let mut hint_offset = 0;

    for instruction in instructions {
        if !instruction.hints.is_empty() {
            // Register hint with string for the hint processor.
            for hint in &instruction.hints {
                string_to_hint.insert(format!("{hint:?}"), hint.clone());
            }
            // Add hint, associated with the instruction offset.
            hints_dict.insert(
                hint_offset,
                instruction.hints.iter().map(hint_to_hint_params).collect(),
            );
        }
        hint_offset += instruction.body.op_size();
    }
    (hints_dict, string_to_hint)
}

pub(crate) fn run_from_test_case(
    runner: &SierraCasmRunner,
    case: &TestCase,
    contracts: &HashMap<String, StarknetContractArtifacts>,
    predeployed_contracts: &Utf8PathBuf,
) -> Result<TestCaseSummary> {
    let available_gas = if let Some(available_gas) = &case.available_gas {
        Some(*available_gas)
    } else {
        Some(usize::MAX)
    };

    let func = runner.find_function(case.name.as_str())?;
    let initial_gas = runner.get_initial_available_gas(func, available_gas)?;
    let (entry_code, builtins) = runner.create_entry_code(func, &[], initial_gas)?;
    let footer = runner.create_code_footer();
    let instructions = chain!(
        entry_code.iter(),
        runner.get_casm_program().instructions.iter(),
        footer.iter()
    );
    let (hints_dict, string_to_hint) = build_hints_dict(instructions.clone());

    // Losely inspired by crates/cheatnet/src/execution/cairo1_execution::execute_entry_point_call_cairo1
    let block_context = build_block_context();
    let account_context = build_transaction_context();
    let mut context = EntryPointExecutionContext::new(
        block_context.clone(),
        account_context,
        block_context.invoke_tx_max_n_steps.try_into().unwrap(),
    );
    let test_selector = get_selector_from_name("TEST_CONTRACT_SELECTOR").unwrap();
    let entry_point_selector = EntryPointSelector(StarkHash::new(test_selector.to_bytes_be())?);
    let entry_point = CallEntryPoint {
        class_hash: None,
        code_address: Some(ContractAddress::from(0_u8)),
        entry_point_type: EntryPointType::External,
        entry_point_selector,
        calldata: Calldata(Arc::new(vec![])),
        storage_address: ContractAddress(patricia_key!("0x0")),
        caller_address: ContractAddress::default(),
        call_type: CallType::Call,
        initial_gas: u64::MAX,
    };

    let mut blockifier_state = CachedState::new(
        build_testing_state(predeployed_contracts),
        GlobalContractCache::default(),
    );
    let mut execution_resources = ExecutionResources::default();
    let syscall_handler = SyscallHintProcessor::new(
        &mut blockifier_state,
        &mut execution_resources,
        &mut context,
        // This segment is created by SierraCasmRunner
        Relocatable {
            segment_index: 10,
            offset: 0,
        },
        entry_point,
        &string_to_hint,
        ReadOnlySegments::default(),
    );
    let mut cairo_hint_processor = CairoHintProcessor {
        blockifier_syscall_handler: syscall_handler,
        contracts,
        cheatnet_state: CheatnetState::new(ExtendedStateReader {
            dict_state_reader: build_testing_state(predeployed_contracts),
            fork_state_reader: None,
        }),
        hints: &string_to_hint,
        run_resources: RunResources::default(),
    };

    match runner.run_function(
        runner.find_function(case.name.as_str())?,
        &mut cairo_hint_processor,
        hints_dict,
        instructions,
        builtins,
    ) {
        Ok(result) => {
            dbg!(execution_resources);
            Ok(TestCaseSummary::from_run_result(result, case))
        }

        // CairoRunError comes from VirtualMachineError which may come from HintException that originates in the cheatcode processor
        Err(RunnerError::CairoRunError(error)) => Ok(TestCaseSummary::Failed {
            name: case.name.clone(),
            run_result: None,
            msg: Some(format!(
                "\n    {}\n",
                error.to_string().replace(" Custom Hint Error: ", "\n    ")
            )),
        }),

        Err(err) => Err(err.into()),
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn run_from_test_case2(
    runner: &SierraCasmRunner,
    case: &TestCase,
    contracts: &HashMap<String, StarknetContractArtifacts>,
    predeployed_contracts: &Utf8PathBuf,
) -> Result<TestCaseSummary> {
    // Code from run_from_test_case
    let available_gas = if let Some(available_gas) = &case.available_gas {
        Some(*available_gas)
    } else {
        Some(usize::MAX)
    };

    let func = runner.find_function(case.name.as_str())?;
    // let initial_gas = runner.get_initial_available_gas(func, available_gas)?;
    // let (entry_code, builtins) = runner.create_entry_code(func, &[], initial_gas)?;
    // let footer = runner.create_code_footer();
    // let instructions = chain!(
    //     entry_code.iter(),
    //     runner.get_casm_program().instructions.iter(),
    //     footer.iter()
    // );
    let builtins = vec![
        BuiltinName::pedersen,
        BuiltinName::range_check,
        BuiltinName::bitwise,
        BuiltinName::ec_op,
        BuiltinName::poseidon,
    ];

    // Building program
    let casm = runner.get_casm_program();
    dbg!(&casm);
    let instructions = casm.instructions.iter().clone();
    let (hints_dict, string_to_hint) = build_hints_dict(instructions);
    let offset = casm.debug_info.sierra_statement_info[func.entry_point.0].code_offset;;
    dbg!(offset);

    let data: Vec<MaybeRelocatable> = casm
        .instructions
        .iter()
        .flat_map(|inst| inst.assemble().encode())
        .map(Felt252::from)
        .map(MaybeRelocatable::from)
        .collect();

    let builtins_ep = builtins
        .clone()
        .iter()
        .map(|bi| bi.name().to_string())
        .collect();
    let program = Program::new(
        builtins,
        data,
        Some(0),
        hints_dict,
        ReferenceManager {
            references: Vec::new(),
        },
        HashMap::new(),
        vec![],
        None,
    )
    .unwrap();

    // Blockifier code

    let block_context = build_block_context();
    let account_context = build_transaction_context();
    let mut context = EntryPointExecutionContext::new(
        block_context.clone(),
        account_context,
        block_context.invoke_tx_max_n_steps.try_into().unwrap(),
    );

    let test_selector = get_selector_from_name("TEST_CONTRACT_SELECTOR").unwrap();
    let entry_point_selector = EntryPointSelector(StarkHash::new(test_selector.to_bytes_be())?);
    let call = CallEntryPoint {
        class_hash: None,
        code_address: Some(ContractAddress::from(0_u8)),
        entry_point_type: EntryPointType::External,
        entry_point_selector,
        calldata: Calldata(Arc::new(vec![])),
        storage_address: ContractAddress(patricia_key!("0x0")),
        caller_address: ContractAddress::default(),
        call_type: CallType::Call,
        initial_gas: u64::MAX,
    };
    let entry_point = EntryPointV1 {
        selector: entry_point_selector,
        offset: EntryPointOffset(offset),
        builtins: builtins_ep,
    };

    let mut blockifier_state = CachedState::new(
        build_testing_state(predeployed_contracts),
        GlobalContractCache::default(),
    );

    let mut execution_resources = ExecutionResources::default();

    let VmExecutionContext {
        mut runner,
        mut vm,
        mut syscall_handler,
        initial_syscall_ptr,
        entry_point,
        program_extra_data_length,
    } = initialize_execution_context(
        call.clone(),
        &program,
        &string_to_hint,
        entry_point,
        &mut blockifier_state,
        &mut execution_resources,
        &mut context,
    )?;

    let args = prepare_call_arguments(
        &syscall_handler.call,
        &mut vm,
        initial_syscall_ptr,
        &mut syscall_handler.read_only_segments,
        &entry_point,
    )?;
    let n_total_args = args.len();

    let previous_vm_resources = syscall_handler.resources.vm_resources.clone();

    let mut cairo_hint_processor = CairoHintProcessor {
        blockifier_syscall_handler: syscall_handler,
        contracts,
        cheatnet_state: CheatnetState::new(ExtendedStateReader {
            dict_state_reader: build_testing_state(predeployed_contracts),
            fork_state_reader: None,
        }),
        hints: &string_to_hint,
        run_resources: RunResources::default(),
    };

    // Execute.
    cheatable_run_entry_point(
        &mut vm,
        &mut runner,
        &mut cairo_hint_processor,
        &entry_point,
        &args,
        program_extra_data_length,
    )?;
    // endregion

    let call_info = finalize_execution(
        vm,
        runner,
        cairo_hint_processor.blockifier_syscall_handler,
        previous_vm_resources,
        n_total_args,
        program_extra_data_length,
    )?;

    dbg!(call_info);

    Ok(TestCaseSummary::Failed {
        name: "aeae".to_string(),
        run_result: None,
        msg: None,
    })
}

fn initialize_execution_context<'a>(
    call: CallEntryPoint,
    // contract_class: &'a ContractClassV1,
    program: &Program,
    hints: &'a HashMap<String, Hint>,
    entry_point: EntryPointV1,
    state: &'a mut dyn State,
    resources: &'a mut ExecutionResources,
    context: &'a mut EntryPointExecutionContext,
) -> Result<VmExecutionContext<'a>, PreExecutionError> {
    // Instantiate Cairo runner.
    let proof_mode = false;
    let mut runner = CairoRunner::new(program, "starknet", proof_mode)?;

    let trace_enabled = false;
    let mut vm = VirtualMachine::new(trace_enabled);

    // Initialize program with all builtins.
    let program_builtins = [
        BuiltinName::bitwise,
        BuiltinName::ec_op,
        BuiltinName::ecdsa,
        BuiltinName::output,
        BuiltinName::pedersen,
        BuiltinName::poseidon,
        BuiltinName::range_check,
        BuiltinName::segment_arena,
    ];
    runner.initialize_function_runner_cairo_1(&mut vm, &program_builtins)?;
    let mut read_only_segments = ReadOnlySegments::default();
    let program_extra_data_length =
        prepare_program_extra_data(&mut vm, program.data_len(), &mut read_only_segments)?;

    // Instantiate syscall handler.
    let initial_syscall_ptr = vm.add_memory_segment();
    let syscall_handler = SyscallHintProcessor::new(
        state,
        resources,
        context,
        initial_syscall_ptr,
        call,
        hints,
        read_only_segments,
    );

    Ok(VmExecutionContext {
        runner,
        vm,
        syscall_handler,
        initial_syscall_ptr,
        entry_point,
        program_extra_data_length,
    })
}

fn prepare_program_extra_data(
    vm: &mut VirtualMachine,
    bytecode_length: usize,
    read_only_segments: &mut ReadOnlySegments,
) -> Result<usize, PreExecutionError> {
    // Create the builtin cost segment, with dummy values.
    let mut data = vec![];

    // TODO(spapini): Put real costs here.
    for _i in 0..20 {
        data.push(MaybeRelocatable::from(0));
    }
    let builtin_cost_segment_start = read_only_segments.allocate(vm, &data)?;

    // Put a pointer to the builtin cost segment at the end of the program (after the
    // additional `ret` statement).
    let mut ptr = (vm.get_pc() + bytecode_length)?;
    // Push a `ret` opcode.
    write_stark_felt(vm, &mut ptr, stark_felt!("0x208b7fff7fff7ffe"))?;
    // Push a pointer to the builtin cost segment.
    write_maybe_relocatable(vm, &mut ptr, builtin_cost_segment_start)?;

    let program_extra_data_length = 2;
    Ok(program_extra_data_length)
}

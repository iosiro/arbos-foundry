//! Concrete execution settings resolved before entering a typed test runner.

use foundry_compilers::ProjectCompileOutput;
use foundry_config::{Config, InlineConfig};
use foundry_evm::{
    core::evm::{ArbitrumEvmFactory, ArbitrumEvmNetwork, FoundryEvmNetwork},
    executors::ExecutorBuilder,
};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

/// Factories for annotated scopes, without runtime execution-family selection.
#[derive(Clone, Debug, Default)]
pub(crate) struct InlineExecutionFactories<F> {
    pub contracts: BTreeMap<String, F>,
    pub functions: BTreeMap<String, BTreeMap<String, F>>,
}

/// Executor construction and already-resolved per-test execution settings.
pub(crate) struct TestExecutorBuilder<FEN: FoundryEvmNetwork> {
    pub executor: ExecutorBuilder<FEN>,
    pub inline_factories: Arc<InlineExecutionFactories<FEN::EvmFactory>>,
}

impl<FEN: FoundryEvmNetwork> From<ExecutorBuilder<FEN>> for TestExecutorBuilder<FEN> {
    fn from(executor: ExecutorBuilder<FEN>) -> Self {
        Self { executor, inline_factories: Default::default() }
    }
}

impl TestExecutorBuilder<ArbitrumEvmNetwork> {
    /// Resolve inline settings once, at the concrete Arbitrum tool boundary.
    pub fn arbitrum(
        config: &Config,
        inline: &InlineConfig,
        output: &ProjectCompileOutput,
        executor: ExecutorBuilder<ArbitrumEvmNetwork>,
    ) -> eyre::Result<Self> {
        let mut factories = InlineExecutionFactories::default();
        for (id, artifact) in output.artifact_ids() {
            let name = id.with_stripped_file_prefixes(&config.root).identifier();
            let contract_config = if inline.contains_contract(&name) {
                Cow::Owned(config.merge_inline_provider(inline.provide(&name, ""))?)
            } else {
                Cow::Borrowed(config)
            };
            if contract_config.stylus != config.stylus {
                factories
                    .contracts
                    .insert(name.clone(), ArbitrumEvmFactory::new(contract_config.stylus.clone()));
            }
            if let Some(abi) = &artifact.abi {
                for function in abi.functions() {
                    if inline.contains_function(&name, &function.name) {
                        let function_config =
                            config.merge_inline_provider(inline.provide(&name, &function.name))?;
                        eyre::ensure!(
                            function_config.stylus.arbos_version
                                == contract_config.stylus.arbos_version,
                            "function-level ArbOS version override in {name}:{} cannot change already-initialized state; configure the version at contract or project scope",
                            function.name,
                        );
                        if function_config.stylus != contract_config.stylus {
                            factories.functions.entry(name.clone()).or_default().insert(
                                function.name.clone(),
                                ArbitrumEvmFactory::new(function_config.stylus),
                            );
                        }
                    }
                }
            }
        }
        Ok(Self { executor, inline_factories: Arc::new(factories) })
    }
}

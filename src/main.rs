// Copyright 2018-2020 Parity Technologies (UK) Ltd.
// This file is part of cargo-contract.
//
// cargo-contract is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// cargo-contract is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with cargo-contract.  If not, see <http://www.gnu.org/licenses/>.

mod cmd;
mod crate_metadata;
#[cfg(test)]
#[cfg(feature = "integration-tests")]
mod tests;
mod util;
mod workspace;

use self::workspace::ManifestPath;

use std::{
    convert::{TryFrom, TryInto},
    path::PathBuf,
    process,
};

use anyhow::{Error, Result};
use colored::Colorize;
use structopt::{clap, StructOpt};

#[cfg(feature = "extrinsics")]
use crate::cmd::{CallCommand, DeployCommand, InstantiateCommand};
#[cfg(feature = "extrinsics")]
use sp_core::{crypto::Pair, sr25519};
#[cfg(feature = "extrinsics")]
use subxt::PairSigner;

#[derive(Debug, StructOpt)]
#[structopt(bin_name = "cargo")]
pub(crate) enum Opts {
    /// Utilities to develop Wasm smart contracts.
    #[structopt(name = "contract")]
    #[structopt(setting = clap::AppSettings::UnifiedHelpMessage)]
    #[structopt(setting = clap::AppSettings::DeriveDisplayOrder)]
    #[structopt(setting = clap::AppSettings::DontCollapseArgsInUsage)]
    Contract(ContractArgs),
}

#[derive(Debug, StructOpt)]
pub(crate) struct ContractArgs {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct HexData(pub Vec<u8>);

#[cfg(feature = "extrinsics")]
impl std::str::FromStr for HexData {
    type Err = hex::FromHexError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        hex::decode(input).map(HexData)
    }
}

/// Arguments required for creating and sending an extrinsic to a substrate node
#[cfg(feature = "extrinsics")]
#[derive(Clone, Debug, StructOpt)]
pub(crate) struct ExtrinsicOpts {
    /// Websockets url of a substrate node
    #[structopt(
        name = "url",
        long,
        parse(try_from_str),
        default_value = "ws://localhost:9944"
    )]
    url: url::Url,
    /// Secret key URI for the account deploying the contract.
    #[structopt(name = "suri", long, short)]
    suri: String,
    /// Password for the secret key
    #[structopt(name = "password", long, short)]
    password: Option<String>,
    #[structopt(flatten)]
    verbosity: VerbosityFlags,
}

#[cfg(feature = "extrinsics")]
impl ExtrinsicOpts {
    pub fn signer(&self) -> Result<PairSigner<subxt::ContractsTemplateRuntime, sr25519::Pair>> {
        let pair =
            sr25519::Pair::from_string(&self.suri, self.password.as_ref().map(String::as_ref))
                .map_err(|_| anyhow::anyhow!("Secret string error"))?;
        Ok(PairSigner::new(pair))
    }

    /// Returns the verbosity
    pub fn verbosity(&self) -> Result<Verbosity> {
        TryFrom::try_from(&self.verbosity)
    }
}

#[derive(Clone, Copy, Debug, StructOpt)]
struct VerbosityFlags {
    #[structopt(long)]
    quiet: bool,
    #[structopt(long)]
    verbose: bool,
}

impl Default for VerbosityFlags {
    fn default() -> Self {
        Self::quiet()
    }
}

impl VerbosityFlags {
    pub fn quiet() -> Self {
        Self {
            quiet: true,
            verbose: false,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Verbosity {
    Quiet,
    Verbose,
    NotSpecified,
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::NotSpecified
    }
}

impl TryFrom<&VerbosityFlags> for Verbosity {
    type Error = Error;

    fn try_from(value: &VerbosityFlags) -> Result<Self, Self::Error> {
        match (value.quiet, value.verbose) {
            (false, false) => Ok(Verbosity::NotSpecified),
            (true, false) => Ok(Verbosity::Quiet),
            (false, true) => Ok(Verbosity::Verbose),
            (true, true) => anyhow::bail!("Cannot pass both --quiet and --verbose flags"),
        }
    }
}

#[derive(Debug, StructOpt)]
struct UnstableOptions {
    /// Use the original manifest (Cargo.toml), do not modify for build optimizations
    #[structopt(long = "unstable-options", short = "Z", number_of_values = 1)]
    options: Vec<String>,
}

#[derive(Clone, Default)]
struct UnstableFlags {
    original_manifest: bool,
}

impl TryFrom<&UnstableOptions> for UnstableFlags {
    type Error = Error;

    fn try_from(value: &UnstableOptions) -> Result<Self, Self::Error> {
        let valid_flags = ["original-manifest"];
        let invalid_flags = value
            .options
            .iter()
            .filter(|o| !valid_flags.contains(&o.as_str()))
            .collect::<Vec<_>>();
        if !invalid_flags.is_empty() {
            anyhow::bail!("Unknown unstable-options {:?}", invalid_flags)
        }
        Ok(UnstableFlags {
            original_manifest: value.options.contains(&"original-manifest".to_owned()),
        })
    }
}

/// Describes which artifacts to generate
#[derive(Copy, Clone, Eq, PartialEq, Debug, StructOpt)]
#[structopt(name = "build-artifacts")]
pub enum GenerateArtifacts {
    /// Generate the Wasm, the metadata and a bundled `<name>.contract` file
    #[structopt(name = "all")]
    All,
    /// Only the Wasm is created, generation of metadata and a bundled `<name>.contract` file is skipped
    #[structopt(name = "code-only")]
    CodeOnly,
}

impl GenerateArtifacts {
    /// Returns the number of steps required to complete a build artifact.
    /// Used as output on the cli.
    pub fn steps(&self) -> usize {
        match self {
            GenerateArtifacts::All => 5,
            GenerateArtifacts::CodeOnly => 3,
        }
    }

    pub fn display(&self, result: &GenerationResult) -> String {
        let optimization = GenerationResult::display_optimization(result);
        let size_diff = format!(
            "\nOriginal wasm size: {}, Optimized: {}\n\n",
            format!("{:.1}K", optimization.0).bold(),
            format!("{:.1}K", optimization.1).bold(),
        );

        if self == &GenerateArtifacts::CodeOnly {
            let out = format!(
                "{}Your contract's code is ready. You can find it here:\n{}",
                size_diff,
                result
                    .dest_wasm
                    .as_ref()
                    .expect("wasm path must exist")
                    .display()
                    .to_string()
                    .bold()
            );
            return out;
        };

        let mut out = format!(
            "{}Your contract artifacts are ready. You can find them in:\n{}\n\n",
            size_diff,
            result.target_directory.display().to_string().bold(),
        );
        if let Some(dest_bundle) = result.dest_bundle.as_ref() {
            let bundle = format!(
                "  - {} (code + metadata)\n",
                GenerationResult::display(&dest_bundle).bold()
            );
            out.push_str(&bundle);
        }
        if let Some(dest_wasm) = result.dest_wasm.as_ref() {
            let wasm = format!(
                "  - {} (the contract's code)\n",
                GenerationResult::display(&dest_wasm).bold()
            );
            out.push_str(&wasm);
        }
        if let Some(dest_metadata) = result.dest_metadata.as_ref() {
            let metadata = format!(
                "  - {} (the contract's metadata)",
                GenerationResult::display(&dest_metadata).bold()
            );
            out.push_str(&metadata);
        }
        out
    }
}

impl std::str::FromStr for GenerateArtifacts {
    type Err = String;
    fn from_str(artifact: &str) -> Result<Self, Self::Err> {
        match artifact {
            "all" => Ok(GenerateArtifacts::All),
            "code-only" => Ok(GenerateArtifacts::CodeOnly),
            _ => Err("Could not parse build artifact".to_string()),
        }
    }
}

/// Result of the metadata generation process.
pub struct GenerationResult {
    /// Path to the resulting metadata file.
    pub dest_metadata: Option<PathBuf>,
    /// Path to the resulting Wasm file.
    pub dest_wasm: Option<PathBuf>,
    /// Path to the bundled file.
    pub dest_bundle: Option<PathBuf>,
    /// Path to the directory where output files are written to.
    pub target_directory: PathBuf,
    /// If existent the result of the optimization.
    pub optimization_result: Option<OptimizationResult>,
}

/// Result of the optimization process.
pub struct OptimizationResult {
    /// The original Wasm size.
    pub original_size: f64,
    /// The Wasm size after optimizations have been applied.
    pub optimized_size: f64,
}

impl GenerationResult {
    /// Returns the base name of the path.
    pub fn display(path: &PathBuf) -> &str {
        path.file_name()
            .expect("file name must exist")
            .to_str()
            .expect("must be valid utf-8")
    }

    /// Returns a tuple of `(original_size, optimized_size)`.
    ///
    /// Panics if no optimization result is available.
    pub fn display_optimization(res: &GenerationResult) -> (f64, f64) {
        let optimization = res
            .optimization_result
            .as_ref()
            .expect("optimization result must exist");
        (optimization.original_size, optimization.optimized_size)
    }
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Setup and create a new smart contract project
    #[structopt(name = "new")]
    New {
        /// The name of the newly created smart contract
        name: String,
        /// The optional target directory for the contract project
        #[structopt(short, long, parse(from_os_str))]
        target_dir: Option<PathBuf>,
    },
    /// Compiles the contract, generates metadata, bundles both together in a `<name>.contract` file
    #[structopt(name = "build")]
    Build {
        /// Path to the Cargo.toml of the contract to build
        #[structopt(long, parse(from_os_str))]
        manifest_path: Option<PathBuf>,
        /// Which build artifacts to generate.
        ///
        /// - `all`: Generate the Wasm, the metadata and a bundled `<name>.contract` file.
        ///
        /// - `code-only`: Only the Wasm is created, generation of metadata and a bundled
        ///   `<name>.contract` file is skipped.
        #[structopt(
            long = "generate",
            default_value = "all",
            value_name = "all | code-only",
            verbatim_doc_comment
        )]
        build_artifact: GenerateArtifacts,
        #[structopt(flatten)]
        verbosity: VerbosityFlags,
        #[structopt(flatten)]
        unstable_options: UnstableOptions,
    },
    /// Command has been deprecated, use `cargo contract build` instead
    #[structopt(name = "generate-metadata")]
    GenerateMetadata {},
    /// Check that the code builds as Wasm; does not output any build artifact to the top level `target/` directory
    #[structopt(name = "check")]
    Check {
        /// Path to the Cargo.toml of the contract to build
        #[structopt(long, parse(from_os_str))]
        manifest_path: Option<PathBuf>,
        #[structopt(flatten)]
        verbosity: VerbosityFlags,
        #[structopt(flatten)]
        unstable_options: UnstableOptions,
    },
    /// Test the smart contract off-chain
    #[structopt(name = "test")]
    Test {},
    /// Upload the smart contract code to the chain
    #[cfg(feature = "extrinsics")]
    #[structopt(name = "deploy")]
    Deploy(DeployCommand),
    /// Instantiate a deployed smart contract
    #[cfg(feature = "extrinsics")]
    Instantiate(InstantiateCommand),
    #[cfg(feature = "extrinsics")]
    Call(CallCommand),
}

fn main() {
    env_logger::init();

    let Opts::Contract(args) = Opts::from_args();
    match exec(args.cmd) {
        Ok(msg) => {
            println!("{}", msg);
            process::exit(0);
        }
        Err(err) => {
            eprintln!(
                "{} {}",
                "ERROR:".bright_red().bold(),
                format!("{:?}", err).bright_red()
            );
            process::exit(1);
        }
    }
}

fn exec(cmd: Command) -> Result<String> {
    match &cmd {
        Command::New { name, target_dir } => cmd::new::execute(name, target_dir.as_ref()),
        Command::Build {
            manifest_path,
            verbosity,
            build_artifact,
            unstable_options,
        } => {
            let manifest_path = ManifestPath::try_from(manifest_path.as_ref())?;
            let result = cmd::build::execute(
                &manifest_path,
                verbosity.try_into()?,
                true,
                *build_artifact,
                unstable_options.try_into()?,
            )?;

            Ok(build_artifact.display(&result))
        }
        Command::Check {
            manifest_path,
            verbosity,
            unstable_options,
        } => {
            let manifest_path = ManifestPath::try_from(manifest_path.as_ref())?;
            let res = cmd::build::execute(
                &manifest_path,
                verbosity.try_into()?,
                false,
                GenerateArtifacts::CodeOnly,
                unstable_options.try_into()?,
            )?;
            assert!(res.dest_wasm.is_none(), "no dest_wasm should exist");
            Ok("\nYour contract's code was built successfully.".to_string())
        }
        Command::GenerateMetadata {} => Err(anyhow::anyhow!(
            "Command deprecated, use `cargo contract build` instead"
        )),
        Command::Test {} => Err(anyhow::anyhow!("Command unimplemented")),
        #[cfg(feature = "extrinsics")]
        Command::Deploy(deploy) => {
            let code_hash = deploy.exec()?;
            Ok(format!("Code hash: {:#x}", code_hash))
        }
        #[cfg(feature = "extrinsics")]
        Command::Instantiate(instantiate) => {
            let contract_account = instantiate.run()?;
            Ok(format!("Contract account: {}", contract_account))
        }
        #[cfg(feature = "extrinsics")]
        Command::Call(call) => call.run(),
    }
}

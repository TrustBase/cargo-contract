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

//! Type definitions for creating and serializing metadata for smart contracts targeting
//! Substrate's contracts pallet.
//!
//! # Example
//!
//! ```
//! # use contract_metadata::*;
//! # use semver::Version;
//! # use url::Url;
//! # use serde_json::{Map, Value};
//!
//! let language = SourceLanguage::new(Language::Ink, Version::new(2, 1, 0));
//! let compiler = SourceCompiler::new(Compiler::RustC, Version::parse("1.46.0-nightly").unwrap());
//! let wasm = SourceWasm::new(vec![0u8]);
//! let source = Source::new(Some(wasm), CodeHash([0u8; 32]), language, compiler);
//! let contract = Contract::builder()
//!     .name("incrementer".to_string())
//!     .version(Version::new(2, 1, 0))
//!     .authors(vec!["Parity Technologies <admin@parity.io>".to_string()])
//!     .description("increment a value".to_string())
//!     .documentation(Url::parse("http://docs.rs/").unwrap())
//!     .repository(Url::parse("http://github.com/paritytech/ink/").unwrap())
//!     .homepage(Url::parse("http://example.com/").unwrap())
//!     .license("Apache-2.0".to_string())
//!     .build()
//!     .unwrap();
//! // user defined raw json
//! let user_json: Map<String, Value> = Map::new();
//! let user = User::new(user_json);
//! // contract abi raw json generated by contract compilation
//! let abi_json: Map<String, Value> = Map::new();
//!
//! let metadata = ContractMetadata::new(source, contract, Some(user), abi_json);
//!
//! // serialize to json
//! let json = serde_json::to_value(&metadata).unwrap();
//! ```

use core::fmt::{Display, Formatter, Result as DisplayResult, Write};
use semver::Version;
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};
use url::Url;

const METADATA_VERSION: &str = "0.1.0";

/// Smart contract metadata.
#[derive(Clone, Debug, Serialize)]
pub struct ContractMetadata {
    #[serde(rename = "metadataVersion")]
    metadata_version: semver::Version,
    source: Source,
    contract: Contract,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<User>,
    /// Raw JSON of the contract abi metadata, generated during contract compilation.
    #[serde(flatten)]
    abi: Map<String, Value>,
}

impl ContractMetadata {
    /// Construct new contract metadata.
    pub fn new(
        source: Source,
        contract: Contract,
        user: Option<User>,
        abi: Map<String, Value>,
    ) -> Self {
        let metadata_version = semver::Version::parse(METADATA_VERSION)
            .expect("METADATA_VERSION is a valid semver string");

        Self {
            metadata_version,
            source,
            contract,
            user,
            abi,
        }
    }

    pub fn remove_source_wasm_attribute(&mut self) {
        self.source.wasm = None;
    }
}

/// Representation of the Wasm code hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeHash(pub [u8; 32]);

impl Serialize for CodeHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_as_byte_str(&self.0[..], serializer)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Source {
    hash: CodeHash,
    language: SourceLanguage,
    compiler: SourceCompiler,
    #[serde(skip_serializing_if = "Option::is_none")]
    wasm: Option<SourceWasm>,
}

impl Source {
    /// Constructs a new InkProjectSource.
    pub fn new(
        wasm: Option<SourceWasm>,
        hash: CodeHash,
        language: SourceLanguage,
        compiler: SourceCompiler,
    ) -> Self {
        Source {
            hash,
            language,
            compiler,
            wasm,
        }
    }
}

/// The bytes of the compiled Wasm smart contract.
#[derive(Clone, Debug)]
pub struct SourceWasm {
    wasm: Vec<u8>,
}

impl SourceWasm {
    /// Constructs a new `SourceWasm`.
    pub fn new(wasm: Vec<u8>) -> Self {
        SourceWasm { wasm }
    }
}

impl Serialize for SourceWasm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_as_byte_str(&self.wasm[..], serializer)
    }
}

impl Display for SourceWasm {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        write!(f, "0x").expect("failed writing to string");
        for byte in &self.wasm {
            write!(f, "{:02x}", byte).expect("failed writing to string");
        }
        write!(f, "")
    }
}

/// The language and version in which a smart contract is written.
#[derive(Clone, Debug)]
pub struct SourceLanguage {
    language: Language,
    version: Version,
}

impl SourceLanguage {
    /// Constructs a new SourceLanguage.
    pub fn new(language: Language, version: Version) -> Self {
        SourceLanguage { language, version }
    }
}

impl Serialize for SourceLanguage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Display for SourceLanguage {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        write!(f, "{} {}", self.language, self.version)
    }
}

/// The language in which the smart contract is written.
#[derive(Clone, Debug)]
pub enum Language {
    Ink,
    Solidity,
    AssemblyScript,
}

impl Display for Language {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        match self {
            Self::Ink => write!(f, "ink!"),
            Self::Solidity => write!(f, "Solidity"),
            Self::AssemblyScript => write!(f, "AssemblyScript"),
        }
    }
}

/// A compiler used to compile a smart contract.
#[derive(Clone, Debug)]
pub struct SourceCompiler {
    compiler: Compiler,
    version: Version,
}

impl Display for SourceCompiler {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        write!(f, "{} {}", self.compiler, self.version)
    }
}

impl Serialize for SourceCompiler {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl SourceCompiler {
    pub fn new(compiler: Compiler, version: Version) -> Self {
        SourceCompiler { compiler, version }
    }
}

/// Compilers used to compile a smart contract.
#[derive(Clone, Debug, Serialize)]
pub enum Compiler {
    RustC,
    Solang,
}

impl Display for Compiler {
    fn fmt(&self, f: &mut Formatter<'_>) -> DisplayResult {
        match self {
            Self::RustC => write!(f, "rustc"),
            Self::Solang => write!(f, "solang"),
        }
    }
}

/// Metadata about a smart contract.
#[derive(Clone, Debug, Serialize)]
pub struct Contract {
    name: String,
    version: Version,
    authors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    documentation: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<String>,
}

impl Contract {
    pub fn builder() -> ContractBuilder {
        ContractBuilder::default()
    }
}

/// Additional user defined metadata, can be any valid json.
#[derive(Clone, Debug, Serialize)]
pub struct User {
    #[serde(flatten)]
    json: Map<String, Value>,
}

impl User {
    /// Constructs new user metadata.
    pub fn new(json: Map<String, Value>) -> Self {
        User { json }
    }
}

/// Builder for contract metadata
#[derive(Default)]
pub struct ContractBuilder {
    name: Option<String>,
    version: Option<Version>,
    authors: Option<Vec<String>>,
    description: Option<String>,
    documentation: Option<Url>,
    repository: Option<Url>,
    homepage: Option<Url>,
    license: Option<String>,
}

impl ContractBuilder {
    /// Set the contract name (required)
    pub fn name<S>(&mut self, name: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        if self.name.is_some() {
            panic!("name has already been set")
        }
        self.name = Some(name.as_ref().to_string());
        self
    }

    /// Set the contract version (required)
    pub fn version(&mut self, version: Version) -> &mut Self {
        if self.version.is_some() {
            panic!("version has already been set")
        }
        self.version = Some(version);
        self
    }

    /// Set the contract version (required)
    pub fn authors<I, S>(&mut self, authors: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if self.authors.is_some() {
            panic!("authors has already been set")
        }

        let authors = authors
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect::<Vec<_>>();

        if authors.len() == 0 {
            panic!("must have at least one author")
        }

        self.authors = Some(authors);
        self
    }

    /// Set the contract description (optional)
    pub fn description<S>(&mut self, description: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        if self.description.is_some() {
            panic!("description has already been set")
        }
        self.description = Some(description.as_ref().to_string());
        self
    }

    /// Set the contract documentation url (optional)
    pub fn documentation(&mut self, documentation: Url) -> &mut Self {
        if self.documentation.is_some() {
            panic!("documentation is already set")
        }
        self.documentation = Some(documentation);
        self
    }

    /// Set the contract repository url (optional)
    pub fn repository(&mut self, repository: Url) -> &mut Self {
        if self.repository.is_some() {
            panic!("repository is already set")
        }
        self.repository = Some(repository);
        self
    }

    /// Set the contract homepage url (optional)
    pub fn homepage(&mut self, homepage: Url) -> &mut Self {
        if self.homepage.is_some() {
            panic!("homepage is already set")
        }
        self.homepage = Some(homepage);
        self
    }

    /// Set the contract license (optional)
    pub fn license<S>(&mut self, license: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        if self.license.is_some() {
            panic!("license has already been set")
        }
        self.license = Some(license.as_ref().to_string());
        self
    }

    /// Finalize construction of the [`ContractMetadata`].
    ///
    /// Returns an `Err` if any required fields missing.
    pub fn build(&self) -> Result<Contract, String> {
        let mut required = Vec::new();

        if let (Some(name), Some(version), Some(authors)) =
            (&self.name, &self.version, &self.authors)
        {
            Ok(Contract {
                name: name.to_string(),
                version: version.clone(),
                authors: authors.to_vec(),
                description: self.description.clone(),
                documentation: self.documentation.clone(),
                repository: self.repository.clone(),
                homepage: self.homepage.clone(),
                license: self.license.clone(),
            })
        } else {
            if self.name.is_none() {
                required.push("name");
            }
            if self.version.is_none() {
                required.push("version")
            }
            if self.authors.is_none() {
                required.push("authors")
            }
            Err(format!(
                "Missing required non-default fields: {}",
                required.join(", ")
            ))
        }
    }
}

/// Serializes the given bytes as byte string.
fn serialize_as_byte_str<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if bytes.is_empty() {
        // Return empty string without prepended `0x`.
        return serializer.serialize_str("");
    }
    let mut hex = String::with_capacity(bytes.len() * 2 + 2);
    write!(hex, "0x").expect("failed writing to string");
    for byte in bytes {
        write!(hex, "{:02x}", byte).expect("failed writing to string");
    }
    serializer.serialize_str(&hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn builder_fails_with_missing_required_fields() {
        let missing_name = Contract::builder()
            // .name("incrementer".to_string())
            .version(Version::new(2, 1, 0))
            .authors(vec!["Parity Technologies <admin@parity.io>".to_string()])
            .build();

        assert_eq!(
            missing_name.unwrap_err(),
            "Missing required non-default fields: name"
        );

        let missing_version = Contract::builder()
            .name("incrementer".to_string())
            // .version(Version::new(2, 1, 0))
            .authors(vec!["Parity Technologies <admin@parity.io>".to_string()])
            .build();

        assert_eq!(
            missing_version.unwrap_err(),
            "Missing required non-default fields: version"
        );

        let missing_authors = Contract::builder()
            .name("incrementer".to_string())
            .version(Version::new(2, 1, 0))
            // .authors(vec!["Parity Technologies <admin@parity.io>".to_string()])
            .build();

        assert_eq!(
            missing_authors.unwrap_err(),
            "Missing required non-default fields: authors"
        );

        let missing_all = Contract::builder()
            // .name("incrementer".to_string())
            // .version(Version::new(2, 1, 0))
            // .authors(vec!["Parity Technologies <admin@parity.io>".to_string()])
            .build();

        assert_eq!(
            missing_all.unwrap_err(),
            "Missing required non-default fields: name, version, authors"
        );
    }

    #[test]
    fn json_with_optional_fields() {
        let language = SourceLanguage::new(Language::Ink, Version::new(2, 1, 0));
        let compiler =
            SourceCompiler::new(Compiler::RustC, Version::parse("1.46.0-nightly").unwrap());
        let wasm = SourceWasm::new(vec![0u8, 1u8, 2u8]);
        let source = Source::new(Some(wasm), CodeHash([0u8; 32]), language, compiler);
        let contract = Contract::builder()
            .name("incrementer".to_string())
            .version(Version::new(2, 1, 0))
            .authors(vec!["Parity Technologies <admin@parity.io>".to_string()])
            .description("increment a value".to_string())
            .documentation(Url::parse("http://docs.rs/").unwrap())
            .repository(Url::parse("http://github.com/paritytech/ink/").unwrap())
            .homepage(Url::parse("http://example.com/").unwrap())
            .license("Apache-2.0".to_string())
            .build()
            .unwrap();

        let user_json = json! {
            {
                "more-user-provided-fields": [
                  "and",
                  "their",
                  "values"
                ],
                "some-user-provided-field": "and-its-value"
            }
        };
        let user = User::new(user_json.as_object().unwrap().clone());
        let abi_json = json! {
            {
                "spec": {},
                "storage": {},
                "types": []
            }
        }
        .as_object()
        .unwrap()
        .clone();

        let metadata = ContractMetadata::new(source, contract, Some(user), abi_json);
        let json = serde_json::to_value(&metadata).unwrap();

        let expected = json! {
            {
                "metadataVersion": "0.1.0",
                "source": {
                    "hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "language": "ink! 2.1.0",
                    "compiler": "rustc 1.46.0-nightly",
                    "wasm": "0x000102"
                },
                "contract": {
                    "name": "incrementer",
                    "version": "2.1.0",
                    "authors": [
                      "Parity Technologies <admin@parity.io>"
                    ],
                    "description": "increment a value",
                    "documentation": "http://docs.rs/",
                    "repository": "http://github.com/paritytech/ink/",
                    "homepage": "http://example.com/",
                    "license": "Apache-2.0",
                },
                "user": {
                    "more-user-provided-fields": [
                      "and",
                      "their",
                      "values"
                    ],
                    "some-user-provided-field": "and-its-value"
                },
                // these fields are part of the flattened raw json for the contract ABI
                "spec": {},
                "storage": {},
                "types": []
            }
        };

        assert_eq!(json, expected);
    }

    #[test]
    fn json_excludes_optional_fields() {
        let language = SourceLanguage::new(Language::Ink, Version::new(2, 1, 0));
        let compiler =
            SourceCompiler::new(Compiler::RustC, Version::parse("1.46.0-nightly").unwrap());
        let source = Source::new(None, CodeHash([0u8; 32]), language, compiler);
        let contract = Contract::builder()
            .name("incrementer".to_string())
            .version(Version::new(2, 1, 0))
            .authors(vec!["Parity Technologies <admin@parity.io>".to_string()])
            .build()
            .unwrap();
        let abi_json = json! {
            {
                "spec": {},
                "storage": {},
                "types": []
            }
        }
        .as_object()
        .unwrap()
        .clone();

        let metadata = ContractMetadata::new(source, contract, None, abi_json);
        let json = serde_json::to_value(&metadata).unwrap();

        let expected = json! {
            {
                "metadataVersion": "0.1.0",
                "source": {
                    "hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "language": "ink! 2.1.0",
                    "compiler": "rustc 1.46.0-nightly"
                },
                "contract": {
                    "name": "incrementer",
                    "version": "2.1.0",
                    "authors": [
                      "Parity Technologies <admin@parity.io>"
                    ],
                },
                // these fields are part of the flattened raw json for the contract ABI
                "spec": {},
                "storage": {},
                "types": []
            }
        };

        assert_eq!(json, expected);
    }
}

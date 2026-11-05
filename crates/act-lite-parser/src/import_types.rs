//! Import statement types and the `ImportHelper` trait.
//!
//! These types describe the import statements parsed from a source file.

use std::path::{Path, PathBuf};

use act_lite_core::Location;

use crate::{SourceFile, Workspace};

/// Represents an import statement in source code.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Import {
    pub module_path: String,
    pub kind: ImportKind,
    pub location: Location,
    pub symbols: Vec<String>,
}

/// The kind of import statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportKind {
    /// ES6 import: `import { x } from 'y'`
    EsImport,
    /// CommonJS require: `require('x')`
    Require,
    /// Dynamic import: `import('x')`
    DynamicImport,
    /// Python import: `import x`
    PythonImport,
    /// Python from import: `from x import y`
    PythonFrom,
    /// Rust use: `use x::y;`
    RustUse,
    /// C# using: `using X.Y;`
    CSharpUsing,
    /// Java import: `import x.y.Z;`
    JavaImport,
    /// Kotlin import: `import com.example.Foo`
    KotlinImport,
    /// Go import: `import "x/y"`
    GoImport,
    /// Swift import: `import Foundation`
    SwiftImport,
    /// Ruby require: `require 'json'`
    RubyRequire,
    /// Ruby require_relative: `require_relative 'lib/foo'`
    RubyRequireRelative,
    /// Elixir alias: `alias MyApp.User`
    ElixirAlias,
    /// Elixir import: `import Enum, only: [map: 2]`
    ElixirImport,
    /// Elixir use: `use GenServer`
    ElixirUse,
    /// Elixir require: `require Logger`
    ElixirRequire,
    /// PHP use declaration: `use App\Models\User`
    PhpUse,
    /// PHP use with alias: `use App\Models\User as UserModel`
    PhpUseAlias,
    /// Scala import: `import com.example.Foo`
    ScalaImport,
    /// Haskell import: `import Data.Map (Map)`
    HaskellImport,
    /// Haskell qualified import: `import qualified Data.Map as Map`
    HaskellQualifiedImport,
    /// Haskell hiding import: `import Data.List hiding (sort)`
    HaskellHidingImport,
    /// Zig @import builtin: `const std = @import("std");`
    ZigImport,
    /// C/C++ preprocessor include: `#include <stdio.h>` or `#include "file.h"`
    CInclude,
    /// Dart import directive: `import 'package:flutter/material.dart';`
    DartImport,
    /// Bash source/dot-source: `source ./lib.sh` or `. ./lib.sh`
    BashSource,
    /// Groovy import: `import java.util.List`
    GroovyImport,
    /// Groovy static import: `import static java.util.Collections.sort`
    GroovyStaticImport,
    /// Perl use declaration: `use Module::Name;` or `use Module qw(func1 func2);`
    PerlUse,
    /// Perl require: `require 'file.pm';`
    PerlRequire,
    /// PowerShell Import-Module: `Import-Module ModuleName`
    PowerShellImportModule,
    /// PowerShell using module: `using module ModuleName`
    PowerShellUsing,
    /// PowerShell dot-sourcing: `. .\path\script.ps1`
    PowerShellDotSource,
    /// Lua require: `require("module")` or `require 'module'`
    LuaRequire,
    /// Common Lisp require/in-package: `(require 'module)` or `(in-package :module)`
    LispRequire,
    /// Erlang include: `-include("file.hrl").`
    ErlangInclude,
    /// Erlang include_lib: `-include_lib("app/include/file.hrl").`
    ErlangIncludeLib,
    /// Erlang import: `-import(module, [func/arity, ...]).`
    ErlangImport,
    /// Fortran use statement: `use module_name` or `use module_name, only: sym1, sym2`
    FortranUse,
    /// F# open statement: `open System`
    FSharpOpen,
    /// Clojure require: `(:require [myapp.util :as util])` in ns form.
    ClojureRequire,
    /// Clojure Java import: `(:import [java.util Date])` or `(import ...)`.
    ClojureImport,
    /// Clojure use: `(:use [clojure.set])` — brings all symbols into scope.
    ClojureUse,
    /// Julia using statement: `using Module`
    JuliaUsing,
    /// Julia import statement: `import Module`
    JuliaImport,
    /// Nim import: `import module` or `from module import symbol`
    NimImport,
    /// OCaml open statement: `open Printf`
    OcamlOpen,
    /// OCaml include statement: `include MyModule`
    OcamlInclude,
    /// ReScript open statement: `open Belt`
    ReScriptOpen,
    /// ReScript include statement: `include Shared`
    ReScriptInclude,
    /// Pascal uses clause: `uses SysUtils, Classes;`
    PascalUses,
    /// VB.NET Imports: `Imports System.Collections.Generic`
    VbNetImports,
    /// R library/require: `library(pkg)` or `require(pkg)`
    RLibrary,
    /// V import: `import module.path`
    VImport,
    /// Jinja import: `{% import "module" as name %}`
    JinjaImport,
    /// Jinja from import: `{% from "module" import symbol %}`
    JinjaFromImport,
    /// Jinja include: `{% include "template" %}`
    JinjaInclude,
    /// CMake include directive: `include(module_or_file)`
    CmakeInclude,
    /// Gleam import: `import gleam/io` or `import gleam/list.{map}`
    GleamImport,
    /// WGSL enable directive: `enable feature;`
    WgslEnable,
    /// Elm import: `import ModuleName`
    ElmImport,
    /// Elm import with alias: `import ModuleName as Alias`
    ElmAlias,
    /// Elm import with selective exposing: `import ModuleName exposing (func1, func2)`
    ElmExposing,
    /// Elm import exposing everything: `import ModuleName exposing (..)`
    ElmExposingAll,
    /// SystemVerilog import: `import package_name::*;`
    SystemVerilogImport,
    /// D import statement: `import std.stdio;`
    DImport,
    /// Odin import: `import "core:fmt"`
    OdinImport,
    /// Bicep module reference: `module name './path.bicep' = { ... }`
    BicepModule,
    /// HCL/Terraform module source: `module "name" { source = "./path" }`
    HclModule,
    /// Rego import: `import data.library`
    RegoImport,
    /// Protocol Buffers import: `import "path/to/file.proto";`
    ProtoImport,
    /// Robot Framework library import: `Library  SeleniumLibrary`
    RobotLibrary,
    /// Robot Framework resource import: `Resource  keywords.robot`
    RobotResource,
    /// Robot Framework variables import: `Variables  vars.py`
    RobotVariables,
}

/// Trait for extracting and resolving import statements.
///
/// Each language implements this trait to handle its specific import syntax
/// and module resolution rules.
pub trait ImportHelper: Send + Sync {
    /// Extracts all imports from the given source code.
    ///
    /// # Arguments
    /// - `content`: The source code content
    /// - `file`: The parsed source file (for AST-based extraction)
    ///
    /// # Returns
    /// A vector of all imports found in the file.
    fn extract_imports(&self, content: &str, file: &SourceFile) -> Vec<Import>;

    /// Resolves an import path to an absolute file path.
    ///
    /// # Arguments
    /// - `import`: The import to resolve
    /// - `source`: The file containing the import
    /// - `workspace`: The workspace for resolution context
    ///
    /// # Returns
    /// The absolute path to the imported file, which must exist on disk,
    /// or `None` when the import cannot be resolved to an existing file.
    /// Returning an unchecked or fabricated path (e.g. the raw module
    /// path, or a relative path that depends on the process working
    /// directory) is a contract violation: implementors must probe for
    /// existence before returning `Some`.
    fn resolve_import_path(
        &self,
        import: &Import,
        source: &Path,
        workspace: &Workspace,
    ) -> Option<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_helper_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn ImportHelper) {}
    }

    #[test]
    fn import_struct_is_constructible() {
        let import = Import {
            module_path: "./utils".into(),
            kind: ImportKind::EsImport,
            location: Location::new(PathBuf::from("test.ts"), 1, 1),
            symbols: vec!["foo".into(), "bar".into()],
        };
        assert_eq!(import.symbols.len(), 2);
        assert_eq!(import.kind, ImportKind::EsImport);
    }

    #[test]
    fn import_kind_variants() {
        assert_ne!(ImportKind::EsImport, ImportKind::Require);
        assert_ne!(ImportKind::PythonImport, ImportKind::RustUse);
        assert_ne!(ImportKind::GoImport, ImportKind::JavaImport);
    }
}

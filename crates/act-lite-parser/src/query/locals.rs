use crate::SourceFile;
use crate::error::Result;
use act_lite_core::Range;
use std::collections::HashSet;
use streaming_iterator::StreamingIterator;

/// One `@local.scope` capture: the byte range a name can bind INTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalScope {
    /// Node id, stable for the tree the model was built from. Definitions
    /// and candidates from the same tree identify their scope by id.
    pub node_id: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// One `@local.definition` capture: a binding site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDefinition {
    /// The bound name, exactly as captured (case-sensitive per grammar).
    pub name: String,
    /// Byte range of the captured node (the name node, not the declaration).
    pub range: Range,
    /// Node id, stable for the tree the model was built from.
    pub node_id: usize,
    /// Node kind of the captured node — the declaration's binding kind the
    /// resolver compares against a candidate's kind.
    pub kind: &'static str,
}

/// The `@local.*` capture set of one file: scopes, definitions, references.
#[derive(Debug, Clone, Default)]
pub struct LocalsModel {
    scopes: Vec<LocalScope>,
    definitions: Vec<LocalDefinition>,
    references: HashSet<usize>,
}

impl LocalsModel {
    /// Builds the model for `source` by running its grammar's locals query.
    ///
    /// `Ok(None)` — the grammar carries no `locals.scm`. `Err` — the query
    /// exists but is invalid for the grammar (unknown node type, bad
    /// pattern): a defect to fix, never a reason to fall back to name
    /// matching.
    pub fn build(source: &SourceFile) -> Result<Option<Self>> {
        let support = source.grammar().language_support();
        let query_source = support.locals_query();
        if query_source.trim().is_empty() {
            return Ok(None);
        }

        let query = tree_sitter::Query::new(&support.ts_language(), query_source)?;
        let tree = source.tree();
        let bytes = tree.source().as_bytes();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), bytes);

        let mut model = LocalsModel::default();
        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let node = capture.node;
                let capture_name = query.capture_names()[capture.index as usize];
                match capture_name {
                    "local.scope" => {
                        model.scopes.push(LocalScope {
                            node_id: node.id(),
                            start_byte: node.start_byte(),
                            end_byte: node.end_byte(),
                        });
                    }
                    "local.definition" => {
                        // The same node can be captured by two patterns
                        // (e.g. C's pointer-declarator rules overlap); a
                        // definition is one node, deduped by id.
                        if model.definitions.iter().any(|d| d.node_id == node.id()) {
                            continue;
                        }
                        model.definitions.push(LocalDefinition {
                            name: node.utf8_text(bytes).unwrap_or_default().trim().to_string(),
                            range: super::node_to_range(node, source.path()),
                            node_id: node.id(),
                            kind: node.kind(),
                        });
                    }
                    "local.reference" => {
                        model.references.insert(node.id());
                    }
                    other => {
                        // A locals.scm carrying a non-standard capture is a
                        // model defect: refuse loudly rather than guess its
                        // meaning.
                        return Err(crate::error::ParserError::QueryFailed(
                            tree_sitter::QueryError {
                                row: 0,
                                column: 0,
                                offset: 0,
                                message: format!(
                                    "locals.scm for {} binds non-standard capture @{other}; \
                                     expected @local.scope/@local.definition/@local.reference",
                                    source.grammar().name()
                                ),
                                kind: tree_sitter::QueryErrorKind::Capture,
                            },
                        ));
                    }
                }
            }
        }
        Ok(Some(model))
    }

    /// The file's scope captures, in document order.
    pub fn scopes(&self) -> &[LocalScope] {
        &self.scopes
    }

    /// The file's definition captures, in document order.
    pub fn definitions(&self) -> &[LocalDefinition] {
        &self.definitions
    }

    /// Whether `node_id` (a node of the tree this model was built from) is
    /// captured as a `@local.reference`. Occurrences outside the reference
    /// allowlist are unmodeled — the resolver refuses them.
    pub fn is_reference(&self, node_id: usize) -> bool {
        self.references.contains(&node_id)
    }

    /// The definition captured AT `node_id`, if the node is a binding site.
    pub fn definition_for_node(&self, node_id: usize) -> Option<&LocalDefinition> {
        self.definitions.iter().find(|d| d.node_id == node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Writes `code` to `<dir>/<stem>` and loads it as a SourceFile. The
    /// caller binds the TempDir for the file's lifetime.
    fn load(dir: &tempfile::TempDir, name: &str, code: &str) -> SourceFile {
        let path: PathBuf = dir.path().join(name);
        std::fs::write(&path, code).unwrap();
        SourceFile::load(&path).unwrap()
    }

    fn def_names<'a>(model: &'a LocalsModel, name: &str) -> Vec<&'a LocalDefinition> {
        model
            .definitions()
            .iter()
            .filter(|d| d.name == name)
            .collect()
    }

    #[test]
    fn unmodeled_grammar_builds_none() {
        let dir = tempfile::tempdir().unwrap();
        let source = load(&dir, "unmodeled.py", "x = 1\n");
        assert!(LocalsModel::build(&source).unwrap().is_none());
    }

    #[test]
    fn c_model_scopes_definitions_references() {
        let dir = tempfile::tempdir().unwrap();
        let code = "int g = 1;\n\
                    int f(int a) { int x = a; { int x = 2; g += x; } return x; }\n\
                    int h(void) { int x = g; return f(x); }\n";
        let source = load(&dir, "model.c", code);
        let model = LocalsModel::build(&source).unwrap().expect("C is modeled");

        // translation_unit + four compound statements (f, inner block, h,
        // and none else) — assert at least the file and the three bodies.
        assert!(model.scopes().len() >= 4, "scopes: {:?}", model.scopes());

        let xs = def_names(&model, "x");
        // f's x, the shadowing inner x, h's x
        assert_eq!(xs.len(), 3, "definitions of x: {:?}", xs);
        let gs = def_names(&model, "g");
        assert_eq!(gs.len(), 1, "global g defined once: {:?}", gs);
        let fs = def_names(&model, "f");
        assert_eq!(fs.len(), 1, "function f named once: {:?}", fs);
        assert!(
            model.definitions().iter().any(|d| d.name == "a"),
            "parameter a modelled: {:?}",
            model.definitions()
        );
        assert!(
            !model.references.is_empty(),
            "identifier uses are references"
        );
    }

    #[test]
    fn rust_model_definitions() {
        let dir = tempfile::tempdir().unwrap();
        let code = "fn f(x: i32) { let y = x; let y = y + 1; y }\n\
                    fn g() { for i in 0..3 { let y = i; } }\n";
        let source = load(&dir, "model.rs", code);
        let model = LocalsModel::build(&source)
            .unwrap()
            .expect("rust is modeled");
        assert_eq!(def_names(&model, "y").len(), 3, "{:?}", model.definitions());
        assert_eq!(def_names(&model, "f").len(), 1);
        assert_eq!(def_names(&model, "i").len(), 1);
    }

    #[test]
    fn go_model_definitions() {
        let dir = tempfile::tempdir().unwrap();
        let code = "package main\n\
                    func f(a int) { x := a; for i := 0; i < 3; i++ { x++ } }\n\
                    func g() { x := 1; _ = x }\n";
        let source = load(&dir, "model.go", code);
        let model = LocalsModel::build(&source).unwrap().expect("go is modeled");
        assert_eq!(def_names(&model, "x").len(), 2, "{:?}", model.definitions());
        assert_eq!(def_names(&model, "i").len(), 1);
        assert_eq!(def_names(&model, "a").len(), 1);
        assert!(model.definitions().iter().any(|d| d.name == "main"));
    }

    #[test]
    fn typescript_model_definitions() {
        let dir = tempfile::tempdir().unwrap();
        let code = "const a = 1;\n\
                    function f(x: number) { const y = x; return y + a; }\n\
                    const g = (p: number) => p + a;\n";
        let source = load(&dir, "model.ts", code);
        let model = LocalsModel::build(&source)
            .unwrap()
            .expect("typescript is modeled");
        assert_eq!(def_names(&model, "a").len(), 1, "{:?}", model.definitions());
        assert_eq!(def_names(&model, "f").len(), 1);
        assert_eq!(def_names(&model, "p").len(), 1);
        // program + f's statement_block + the arrow function (whose body is
        // an expression, so no statement_block of its own)
        assert!(model.scopes().len() >= 3, "{:?}", model.scopes());
    }

    #[test]
    fn javascript_model_definitions() {
        let dir = tempfile::tempdir().unwrap();
        let code = "let a = 1;\n\
                    function f(x) { let y = x; return y + a; }\n\
                    for (const item of [1, 2]) { a += item; }\n";
        let source = load(&dir, "model.js", code);
        let model = LocalsModel::build(&source)
            .unwrap()
            .expect("javascript is modeled");
        assert_eq!(def_names(&model, "a").len(), 1, "{:?}", model.definitions());
        assert_eq!(def_names(&model, "item").len(), 1);
    }

    #[test]
    fn every_modeled_grammar_compiles_its_query() {
        // Every locals.scm must compile against its grammar — query defects
        // fail SILENTLY otherwise (wrong node kinds still match nothing).
        let dir = tempfile::tempdir().unwrap();
        let cases: &[(&str, &str)] = &[
            ("all.c", "int f(void) { int x = 0; return x; }\n"),
            ("all.rs", "fn f() { let x = 0; x }\n"),
            ("all.go", "package main\nfunc main() { x := 0; _ = x }\n"),
            ("all.ts", "const x = 0; x;\n"),
            ("all.js", "const x = 0; x;\n"),
        ];
        for (name, code) in cases {
            let source = load(&dir, name, code);
            let model = LocalsModel::build(&source)
                .unwrap_or_else(|e| panic!("{name}: locals query invalid: {e}"));
            assert!(model.is_some(), "{name}: expected a model");
        }
    }
}

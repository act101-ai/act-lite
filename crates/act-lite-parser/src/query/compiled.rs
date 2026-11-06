//! Compiled tree-sitter queries.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::Grammar;

type Cache = HashMap<Grammar, HashMap<String, Arc<tree_sitter::Query>>>;

/// The query `source` compiled for `grammar`, compiled once per process and
/// reused: compiling a large query set (Zig, Julia) costs a tenth of a second
/// or more, far above the cost of running it. A query that does not compile
/// is not cached and returns its error on each call.
pub(crate) fn compiled(
    grammar: Grammar,
    source: &str,
) -> Result<Arc<tree_sitter::Query>, tree_sitter::QueryError> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    // The lock is held while compiling, so two threads never compile the
    // same query.
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let queries = cache.entry(grammar).or_default();
    if let Some(query) = queries.get(source) {
        return Ok(Arc::clone(query));
    }
    #[cfg(test)]
    count::add(grammar, source);
    let query = Arc::new(tree_sitter::Query::new(
        &grammar.language_support().ts_language(),
        source,
    )?);
    queries.insert(source.to_string(), Arc::clone(&query));
    Ok(query)
}

#[cfg(test)]
pub(crate) mod count {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use crate::Grammar;

    fn counts() -> &'static Mutex<HashMap<(Grammar, String), usize>> {
        static COUNTS: OnceLock<Mutex<HashMap<(Grammar, String), usize>>> = OnceLock::new();
        COUNTS.get_or_init(Default::default)
    }

    pub(crate) fn add(grammar: Grammar, source: &str) {
        *counts()
            .lock()
            .unwrap()
            .entry((grammar, source.to_string()))
            .or_default() += 1;
    }

    /// How many times `source` was compiled for `grammar` in this process.
    pub(crate) fn of(grammar: Grammar, source: &str) -> usize {
        counts()
            .lock()
            .unwrap()
            .get(&(grammar, source.to_string()))
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Grammar, SourceFile};

    /// Each navigation query is compiled once per grammar and process, not
    /// once per call: compiling the Zig queries costs about 0.37 s.
    #[test]
    fn each_query_is_compiled_once_per_grammar() {
        let support = Grammar::Zig.language_support();
        for (i, text) in ["const a = 1;\n", "fn f() void {}\n"].iter().enumerate() {
            let source =
                SourceFile::from_content(format!("f{i}.zig"), text.to_string(), Grammar::Zig)
                    .unwrap();
            crate::execute_skeleton(&source).unwrap();
            crate::execute_symbols(&source).unwrap();
            crate::execute_references(&source).unwrap();
            crate::execute_raw_query(&source, support.references_query()).unwrap();
        }
        for query in [
            support.skeleton_query(),
            support.symbols_query(),
            support.references_query(),
        ] {
            assert_eq!(super::count::of(Grammar::Zig, query), 1);
        }
    }

    /// A raw query that is not one of the grammar's own query files is not
    /// kept: arbitrary query text must not accumulate for the process.
    #[test]
    fn a_custom_raw_query_is_not_cached() {
        let custom = "(identifier) @custom_raw_query_probe";
        let source =
            SourceFile::from_content("c.zig", "const a = 1;\n".to_string(), Grammar::Zig).unwrap();
        crate::execute_raw_query(&source, custom).unwrap();
        crate::execute_raw_query(&source, custom).unwrap();
        assert_eq!(super::count::of(Grammar::Zig, custom), 0);
    }
}

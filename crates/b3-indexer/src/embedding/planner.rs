use std::collections::BTreeMap;

use b3_core::{IndexedFileRecord, SourceKind, VectorDocument};

use super::chunking::{ChunkPlanner, ChunkPlannerConfig, ChunkSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingPlan {
    pub documents: Vec<VectorDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingPlanner {
    chunk_planner: ChunkPlanner,
}

impl EmbeddingPlanner {
    pub fn new(config: ChunkPlannerConfig) -> Self {
        Self {
            chunk_planner: ChunkPlanner::new(config),
        }
    }

    pub fn plan_indexed_file(
        &self,
        file: &IndexedFileRecord,
        branch_id: &b3_core::BranchId,
    ) -> EmbeddingPlan {
        let mut documents = Vec::new();
        let mut symbols = file.symbols.clone();
        symbols.sort_by(|left, right| {
            left.start_line
                .cmp(&right.start_line)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });

        for symbol in symbols {
            let snippet = symbol_text(file, symbol.start_line, symbol.end_line);
            if snippet.trim().is_empty() {
                continue;
            }
            documents.extend(self.chunk_planner.plan_source(ChunkSource {
                project_id: file.file.project_id.clone(),
                branch_id: branch_id.clone(),
                file_id: file.file.id.clone(),
                symbol_id: Some(symbol.id.clone()),
                language: file.language.clone(),
                framework: None,
                source_kind: SourceKind::SymbolChunk,
                path: file.file.path.clone(),
                content_hash: file.file.content_hash.clone(),
                text: snippet,
                start_line: symbol.start_line.max(1),
                metadata: BTreeMap::from([
                    ("symbol_name".to_string(), symbol.name),
                    ("symbol_kind".to_string(), format!("{:?}", symbol.kind)),
                ]),
            }));
        }

        if documents.is_empty() {
            documents.extend(self.chunk_planner.plan_source(ChunkSource {
                project_id: file.file.project_id.clone(),
                branch_id: branch_id.clone(),
                file_id: file.file.id.clone(),
                symbol_id: None,
                language: file.language.clone(),
                framework: None,
                source_kind: SourceKind::FileChunk,
                path: file.file.path.clone(),
                content_hash: file.file.content_hash.clone(),
                text: file.content.clone(),
                start_line: 1,
                metadata: BTreeMap::new(),
            }));
        }

        EmbeddingPlan { documents }
    }
}

fn symbol_text(file: &IndexedFileRecord, start_line: usize, end_line: usize) -> String {
    let start = start_line.max(1);
    let end = end_line.max(start);
    file.content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            (line_number >= start && line_number <= end).then_some(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use b3_core::{
        BranchId, FileId, FileRecord, IndexedFileRecord, NodeKind, ProjectId, SymbolId,
        SymbolRecord,
    };

    use super::*;

    fn file(symbols: Vec<SymbolRecord>) -> IndexedFileRecord {
        IndexedFileRecord {
            file: FileRecord {
                id: FileId::new("file"),
                project_id: ProjectId::new("project"),
                path: "src/lib.rs".to_string(),
                content_hash: "hash".to_string(),
            },
            language: Some("rust".to_string()),
            size_bytes: 32,
            content: "pub fn alpha() {}\n\npub fn beta() {}\n".to_string(),
            symbols,
            edges: Vec::new(),
        }
    }

    #[test]
    fn prefers_symbol_chunks_in_stable_order() {
        let planner = EmbeddingPlanner::new(ChunkPlannerConfig::default());
        let mut beta = SymbolRecord::new(
            SymbolId::new("beta"),
            FileId::new("file"),
            "beta",
            NodeKind::Function,
        );
        beta.start_line = 3;
        beta.end_line = 3;
        let mut alpha = SymbolRecord::new(
            SymbolId::new("alpha"),
            FileId::new("file"),
            "alpha",
            NodeKind::Function,
        );
        alpha.start_line = 1;
        alpha.end_line = 1;

        let plan = planner.plan_indexed_file(&file(vec![beta, alpha]), &BranchId::new("main"));

        assert_eq!(plan.documents.len(), 2);
        assert_eq!(plan.documents[0].metadata["symbol_name"], "alpha");
        assert_eq!(plan.documents[1].metadata["symbol_name"], "beta");
        assert!(plan
            .documents
            .iter()
            .all(|document| document.source_kind == SourceKind::SymbolChunk));
    }

    #[test]
    fn falls_back_to_file_chunks_when_no_symbols_exist() {
        let planner = EmbeddingPlanner::new(ChunkPlannerConfig::default());
        let plan = planner.plan_indexed_file(&file(Vec::new()), &BranchId::new("main"));

        assert_eq!(plan.documents.len(), 1);
        assert_eq!(plan.documents[0].source_kind, SourceKind::FileChunk);
    }
}

use std::collections::BTreeMap;

use b3_core::{
    BranchId, FileId, ProjectId, SourceKind, SymbolId, VectorDocument, VectorDocumentInput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkPlannerConfig {
    pub max_chunk_chars: usize,
}

impl Default for ChunkPlannerConfig {
    fn default() -> Self {
        Self {
            max_chunk_chars: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSource {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub symbol_id: Option<SymbolId>,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub source_kind: SourceKind,
    pub path: String,
    pub content_hash: String,
    pub text: String,
    pub start_line: usize,
    pub metadata: BTreeMap<String, String>,
}

impl ChunkSource {
    pub fn file(
        project_id: ProjectId,
        branch_id: BranchId,
        file_id: FileId,
        path: impl Into<String>,
        content_hash: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            project_id,
            branch_id,
            file_id,
            symbol_id: None,
            language: None,
            framework: None,
            source_kind: SourceKind::FileChunk,
            path: path.into(),
            content_hash: content_hash.into(),
            text: text.into(),
            start_line: 1,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkCandidate {
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkPlanner {
    config: ChunkPlannerConfig,
}

impl ChunkPlanner {
    pub fn new(config: ChunkPlannerConfig) -> Self {
        Self { config }
    }

    pub fn plan_source(&self, source: ChunkSource) -> Vec<VectorDocument> {
        let chunks = self.split_text(&source.text, source.start_line);
        chunks
            .into_iter()
            .enumerate()
            .map(|(chunk_index, chunk)| {
                VectorDocument::new(VectorDocumentInput {
                    project_id: source.project_id.clone(),
                    branch_id: source.branch_id.clone(),
                    file_id: source.file_id.clone(),
                    symbol_id: source.symbol_id.clone(),
                    language: source.language.clone(),
                    framework: source.framework.clone(),
                    source_kind: source.source_kind,
                    path: source.path.clone(),
                    content_hash: source.content_hash.clone(),
                    chunk_index,
                    text: chunk.text,
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    metadata: source.metadata.clone(),
                })
            })
            .collect()
    }

    pub fn split_text(&self, text: &str, start_line: usize) -> Vec<ChunkCandidate> {
        let max_chars = self.config.max_chunk_chars.max(1);
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_start_line = start_line;
        let mut current_end_line = start_line;

        for (line_offset, line) in text.lines().enumerate() {
            let line_number = start_line + line_offset;
            if line.trim().is_empty() && current.trim().is_empty() {
                continue;
            }

            let line_with_newline = format!("{line}\n");
            if !current.is_empty() && current.len() + line_with_newline.len() > max_chars {
                push_chunk(
                    &mut chunks,
                    &mut current,
                    current_start_line,
                    current_end_line,
                );
                current_start_line = line_number;
            }

            if line_with_newline.len() > max_chars {
                if !current.is_empty() {
                    push_chunk(
                        &mut chunks,
                        &mut current,
                        current_start_line,
                        current_end_line,
                    );
                }
                for slice in split_long_line(&line_with_newline, max_chars) {
                    chunks.push(ChunkCandidate {
                        text: slice,
                        start_line: line_number,
                        end_line: line_number,
                    });
                }
                current_start_line = line_number + 1;
                current_end_line = line_number + 1;
                continue;
            }

            current.push_str(&line_with_newline);
            current_end_line = line_number;
        }

        if !current.trim().is_empty() {
            push_chunk(
                &mut chunks,
                &mut current,
                current_start_line,
                current_end_line,
            );
        }

        chunks
    }
}

fn push_chunk(
    chunks: &mut Vec<ChunkCandidate>,
    current: &mut String,
    start_line: usize,
    end_line: usize,
) {
    let text = current.trim_end().to_string();
    if !text.trim().is_empty() {
        chunks.push(ChunkCandidate {
            text,
            start_line,
            end_line,
        });
    }
    current.clear();
}

fn split_long_line(line: &str, max_chars: usize) -> Vec<String> {
    let chars = line.chars().collect::<Vec<_>>();
    chars
        .chunks(max_chars)
        .map(|chunk| chunk.iter().collect::<String>().trim_end().to_string())
        .filter(|chunk| !chunk.trim().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(text: &str) -> ChunkSource {
        ChunkSource {
            project_id: ProjectId::new("project"),
            branch_id: BranchId::new("main"),
            file_id: FileId::new("file"),
            symbol_id: Some(SymbolId::new("symbol")),
            language: Some("rust".to_string()),
            framework: None,
            source_kind: SourceKind::SymbolChunk,
            path: "src/lib.rs".to_string(),
            content_hash: "hash".to_string(),
            text: text.to_string(),
            start_line: 10,
            metadata: BTreeMap::from([("symbol".to_string(), "run".to_string())]),
        }
    }

    #[test]
    fn chunk_ids_and_ordering_are_stable() {
        let planner = ChunkPlanner::new(ChunkPlannerConfig {
            max_chunk_chars: 20,
        });
        let first = planner.plan_source(source("alpha\nbeta\ngamma\ndelta\n"));
        let second = planner.plan_source(source("alpha\nbeta\ngamma\ndelta\n"));

        assert_eq!(first, second);
        assert!(first
            .windows(2)
            .all(|pair| pair[0].chunk_index < pair[1].chunk_index));
    }

    #[test]
    fn respects_max_chunk_chars_and_preserves_line_ranges() {
        let planner = ChunkPlanner::new(ChunkPlannerConfig {
            max_chunk_chars: 12,
        });
        let chunks = planner.plan_source(source("alpha\nbeta\ngamma\n"));

        assert!(chunks.iter().all(|chunk| chunk.text.len() <= 12));
        assert_eq!(chunks[0].start_line, 10);
        assert!(chunks.last().expect("last").end_line >= 12);
    }

    #[test]
    fn skips_empty_content_and_preserves_source_kind_metadata() {
        let planner = ChunkPlanner::new(ChunkPlannerConfig::default());
        let empty = planner.plan_source(source("\n\n   \n"));
        let chunks = planner.plan_source(source("pub fn run() {}\n"));

        assert!(empty.is_empty());
        assert_eq!(chunks[0].source_kind, SourceKind::SymbolChunk);
        assert_eq!(chunks[0].metadata.get("symbol").expect("symbol"), "run");
    }
}

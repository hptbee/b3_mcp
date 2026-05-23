//! Stable identifiers shared across storage, indexing, query, MCP, and UI.

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_id!(ProjectId);
string_id!(FileId);
string_id!(NodeId);
string_id!(EdgeId);
string_id!(SymbolId);
string_id!(BranchId);
string_id!(SessionId);
string_id!(ToolCallId);
string_id!(PluginId);

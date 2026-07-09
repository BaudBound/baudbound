use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeConditionRow {
    #[serde(default)]
    pub(crate) invert: bool,
    pub(crate) left: String,
    #[serde(default)]
    pub(crate) combinator: Option<String>,
    pub(crate) operator: String,
    pub(crate) right: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeSwitchCaseRow {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) value: Option<String>,
    #[serde(default, alias = "expectedValue")]
    pub(crate) expected_value: Option<String>,
}

pub(crate) enum RuntimeFrame {
    Follow {
        source_node_id: String,
        handle: String,
        stop_at_node_id: Option<String>,
    },
    ForEach {
        node_id: String,
        index: usize,
        items: Vec<Value>,
    },
    Loop {
        node_id: String,
        index: u64,
        count: u64,
    },
    Node {
        node_id: String,
        stop_at_node_id: Option<String>,
    },
    While {
        node_id: String,
        index: u64,
    },
}

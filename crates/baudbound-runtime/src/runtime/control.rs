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
    #[serde(default, rename = "rightEnd")]
    pub(crate) right_end: String,
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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeRouterPort {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) label: String,
}

/// The route `id` is an editor-side handle for the panel; the runtime never
/// reads it, so it is left out and serde ignores it.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeRouterRoute {
    #[serde(rename = "inputId")]
    pub(crate) input_id: String,
    #[serde(rename = "outputId")]
    pub(crate) output_id: String,
    pub(crate) order: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeRouterConfig {
    pub(crate) inputs: Vec<RuntimeRouterPort>,
    pub(crate) outputs: Vec<RuntimeRouterPort>,
    pub(crate) routes: Vec<RuntimeRouterRoute>,
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
    Repeat {
        node_id: String,
        index: u64,
        count: u64,
    },
    Node {
        node_id: String,
        input_handle: Option<String>,
        stop_at_node_id: Option<String>,
    },
    While {
        node_id: String,
        index: u64,
    },
}

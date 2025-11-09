use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct SimulationRun {
    pub description: String,
    pub required: bool,
    pub pre_world: Option<PreWorld>,
    pub timeline: Vec<Tick>,
    pub assert_state: Option<AssertState>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Tick {
    pub tick: i64,
    pub action: Option<Action>,
    pub assert_state: Option<AssertState>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub method: String,
    pub slot: i32,
    pub item: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AssertState {
    pub block_id: String,
    pub cords: Vec<i32>,
    pub properties: BlockProperties,
    pub inventory: Option<Vec<InventorySlot>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PreWorld {
    pub blocks: Vec<BlockPosition>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BlockPosition {
    pub cords: Vec<i32>,
    pub block_id: String,
    pub properties: BlockProperties,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BlockProperties {
    pub burning: Option<bool>,
    pub waterlogged: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct InventorySlot {
    pub item: String,
    pub amount: String,
}
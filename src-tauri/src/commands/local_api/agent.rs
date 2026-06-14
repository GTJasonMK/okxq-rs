// Agent API — AI 助手查询和分析端点
// 为 AI 模型提供结构化的市场数据、技术指标、风险分析等查询能力

use serde_json::{json, Value};

use crate::app_state::AppState;
use crate::commands::local_api::infer_inst_type;
use crate::error::{AppError, AppResult};

mod analysis;
mod capabilities;
mod queries;
mod scope;
mod structure;

pub(super) use self::analysis::*;
pub(super) use self::capabilities::*;
pub(super) use self::queries::*;
pub(super) use self::structure::*;

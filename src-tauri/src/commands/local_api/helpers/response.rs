use serde_json::{json, Value};

pub(crate) struct ResponseOutcome {
    pub(crate) success: bool,
    pub(crate) code: i64,
    pub(crate) message: Option<String>,
}

pub(crate) fn response_outcome(value: &Value) -> ResponseOutcome {
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string);
    ResponseOutcome {
        success: code == 0,
        code,
        message,
    }
}

pub(crate) trait WithMessage {
    fn with_message(self, message: &str) -> Value;
}

impl WithMessage for Value {
    fn with_message(mut self, message: &str) -> Value {
        if let Some(obj) = self.as_object_mut() {
            obj.insert("message".to_string(), Value::String(message.to_string()));
        }
        self
    }
}

pub(crate) fn code_ok(data: Value) -> Value {
    json!({"code": 0, "message": "success", "data": data})
}

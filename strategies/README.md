# Runtime Strategy Contract

策略运行文件放在 `strategies/` 下，后端会自动发现根目录和 `strategies/runtime/` 中的 `.py` 文件。`strategy_records/` 只用于研究复现归档，不参与运行时注册。

## Runtime/Research Boundary

Runtime strategies must be loadable without importing from `scripts/research`.
Research code can be copied into `strategies/` after a candidate is promoted,
but the copied code then becomes runtime-owned and should keep its own frozen
contract, tests, and artifact references.

Use `strategies/lib/` for helper modules that are shared by runtime strategy
files but are not standalone strategies. The strategy scanner only registers
`.py` files directly under `strategies/` and `strategies/runtime/`, so helper
modules in `strategies/lib/` do not appear as selectable strategies.

For ML selector strategies, the system runtime should provide raw point-in-time
context declared by `DATA_REQUIREMENTS`: candles, funding, order books,
positions, account state, and order state. Candidate-trade generation, feature
construction, model loading, model scoring, and top-k selection belong to the
strategy runtime code under `strategies/`, not to the mutable research platform
and not to the Rust execution core.

## Required Module Fields

- `STRATEGY_ID`: stable machine id.
- `STRATEGY_NAME`: display name.
- `STRATEGY_TYPE`: strategy shape, for example `single_symbol_strategy`.
- `DATA_REQUIREMENTS`: structured data needs for context collection.
- `RUNTIME_CONFIG`: frozen runtime configuration tuned during research.
- `VISUALIZATION`: metadata describing chart series and diagnostics.
- `DECISION_CONTRACT`: stable decision/side/reason semantics.
- `evaluate(context, params)`: required decision entrypoint. Returns `StrategyDecision`, including `execution_logs`.
- `compute_indicators(candles, params)`: optional chart series helper.

`evaluate()` receives the same canonical context shape in backtest and simulated/live paths:

```python
{
    "candles": {
        "BTC-USDT-SWAP": {
            "15m": [{"timestamp": 1780290000000, "open": 1, "high": 1, "low": 1, "close": 1, "volume": 1}]
        }
    },
    "funding": {},
    "orderbook": {},
    "positions": {},
    "account": {},
    "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
    "time": {"timestamp": 1780290000000, "timeframe": "15m"},
    "runtime": {
        "strategy_id": "runtime_candidate_breakout_v1",
        "symbol": "BTC-USDT-SWAP",
        "inst_type": "SWAP",
        "timeframe": "15m",
    },
}
```

`StrategyDecision`:

```python
{
    "actions": [
        {
            "action": "open_position" | "close_position" | "place_risk_order" | "cancel_order" | "modify_order" | "hold",
            "symbol": "BTC-USDT-SWAP",
            "side": "long" | "short" | "flat",
            "order_type": "market",
            "target_order_kind": "any" | "exchange" | "algo",  # optional for cancel_order/modify_order
            "target_order_type": "stop_market",        # external algo modify only; stop/take-profit market protective type
            "reference_price": 123.45,
            "price_source": "strategy_reference",
            "reason": "stable_reason_code",
            "strength": 0.0,
            "position_size": 0.2,
            "timestamp": 1780290000000,
        }
    ],
    "diagnostics": {
        "action_intent": "open_position",
        "action_count": 1,
        "open_action_count": 1,
        "risk_action_count": 0,
        "summary": "Selected one executable action.",
    },
    "indicators": {},
    "execution_logs": [
        {
            "stage": "strategy_decision",
            "level": "info",
            "message": "Strategy evaluated latest context",
            "details": {"candidate_count": 12, "selected_count": 1},
        }
    ],
}
```

`execution_logs` is part of the runtime strategy contract. Every new or promoted
strategy must return this field from `evaluate()` and `evaluate_history()`; use an
empty list only when the strategy genuinely has no internal stage worth reporting.
`diagnostics` must also be returned explicitly as a dict; use `{}` only when the
strategy genuinely has no diagnostic evidence to expose.
Each log entry must be structured:

```python
{
    "stage": "strategy_input" | "candidate_generation" | "model_scoring" | "strategy_decision" | "...",
    "level": "info" | "warn" | "error" | "success",
    "message": "human-readable short message",
    "details": {},
}
```

Shorthand or alias fields such as string log entries, `summary`, `detail`,
`phase`, `severity`, or `data` are not part of the contract and are rejected by
the runner.

Strategies should build entries with `strategies/lib/runtime_logging.py`:

```python
from lib.runtime_logging import emit_execution_log, execution_log

logs = [
    execution_log("strategy_input", "Loaded 1200 candles", "info", {"candles": 1200})
]
emit_execution_log(context, "strategy_input", logs[-1]["message"], "info", logs[-1]["details"])
return {"actions": actions, "diagnostics": diagnostics, "indicators": indicators, "execution_logs": logs}
```

`emit_execution_log()` writes live `{"event":"strategy_log", ...}` stdout events
only when the runtime enables `context["runtime"]["strategy_log_events"]`. This is
the project-level interface for long-running strategy internals; generic `print`
or stderr output is not a valid strategy log contract.

When a strategy reports progress through
`diagnostics.backtest_progress`, `diagnostics.task_progress`, or
`diagnostics.run_progress`, the payload must use `progress`, `stage`, and
`message` explicitly. Aliases such as `pct`, `percent`, `phase`, `summary`, and
`detail` are not parsed.

For selector/top-k style strategies, live/simulated execution consumes only the
top-level `actions` list. Top-level `orders` and `risk_orders` are not part of
the runtime contract and are rejected by the runner. Protective risk orders must
be returned as `place_risk_order` actions so the runtime can attach matching OKX
TP/SL algos when possible. The runtime exposes persisted order state through
`context["orders"]` on later evaluations.

The runner does not canonicalize old action aliases. Every action object must
contain an `action` field with one of the canonical values above; fields such as
`intent_action`, `signal_type`, and `type` are rejected as protocol errors.

Order state rows are normalized for backtest and simulated/live private REST/WS.
Strategies can rely on `symbol`/`inst_id`, `side`, `order_type`, `price`,
`quantity`, `size`, `value`, `status`, `success`, `action`,
`error_message`, and `timestamp` when present. In order state rows, `quantity`
and `size` denote the filled or submitted amount; pending backtest risk orders
expose the strategy-provided intent size when no exchange/order fill quantity
exists yet. `recent_rejections[].status` carries values such as `risk_blocked`,
`slippage_blocked`, `blocked`, or exchange terminal states.

```python
{
    "actions": [
        {
            "action": "open_position",
            "symbol": "SOL-USDT-SWAP",
            "side": "long",
            "order_type": "market",
            "reference_price": 123.45,
            "price_source": "strategy_reference",
            "position_size": 0.12,
            "planned_exit_time": 1780290900000,
            "reason": "ml_rank_1",
        },
        {
            "action": "place_risk_order",
            "symbol": "SOL-USDT-SWAP",
            "side": "sell",
            "order_type": "stop_market",
            "trigger_price": 116.04,
            "stop_loss_bps": 600,
            "max_slippage_bps": 20,
            "reason": "protective_stop",
        },
    ],
    "diagnostics": {
        "action_intent": "open_position",
        "action_count": 2,
        "open_action_count": 1,
        "risk_action_count": 1,
    },
    "indicators": {},
    "execution_logs": [],
}
```

`stop_loss_bps` / `take_profit_bps` / `max_slippage_bps` are action-level
risk hints. Decimal forms are also accepted: `stop_loss_pct` / `stop_loss`,
`take_profit_pct` / `take_profit`, and `max_slippage_pct` / `max_slippage`.
`price` is the strategy's explicit order price and is required only for
price-required OKX order types such as `limit`, `post_only`, `fok`, and `ioc`.
Market actions must return an explicit `price` or `reference_price` for sizing,
risk checks, slippage checks, and logs. The Python
runner does not infer a missing market reference price from the latest candle.
`price_source` may be `explicit`, `strategy_reference`, `arrival_mid`, or
another stable strategy-defined source label.
`position_size` is the only action size field for strategy position sizing.
`size`, `quantity`, `qty`, and `sz` are not accepted in strategy output. To
submit an exact OKX `sz`, use `exchange_size`. Explicit exchange sizes are
validated against OKX `lotSz` and
`minSz` before submission and are rejected instead of being silently rounded or
capped, except that internal planned-exit residual sizing is still managed by
the runtime.
Backtest applies `max_slippage_bps` against configured simulated slippage;
live/simulated runtime applies it against the latest arrival bid/ask before an
entry order is submitted. If a strategy explicitly sets a max slippage guard and
no usable bid/ask is available, the order is rejected as `slippage_blocked`.

Execution numeric fields in action objects must be native JSON numbers, not
strings containing numbers. This applies to `timestamp`, `price`,
`reference_price`, `position_size`, `strength`, `planned_exit_time`,
`trigger_price`, `stop_loss_bps`, `stop_loss_pct`, `stop_loss`,
`take_profit_bps`, `take_profit_pct`, `take_profit`, `max_slippage_bps`,
`max_slippage_pct`, and `max_slippage`. The runner rejects or marks invalid
string numerics instead of silently coercing them, because silent coercion can
change order price, quantity, risk, or planned-exit behavior in live/simulated
execution. Action semantic text fields such as `action`, `side`, `order_type`,
`order_side`, `close_side`, `reason`, `inst_type`, and `timeframe`, plus textual
OKX identity/size fields such as `exchange_size`, `order_id`, `client_order_id`,
`new_size`, `new_price`, and `request_id`, must be returned as JSON strings; the
runner and Rust planner do not coerce JSON numbers or booleans into order
semantics, exchange identifiers, or exact exchange sizes.

Diagnostics must describe the canonical action contract, not old signal UI
state. Use fields such as `action_intent`, `action_count`,
`open_action_count`, `risk_action_count`, `selected_count`, `candidate_count`,
`blocked_by`, and `summary`. Do not return old visualisation fields such as
`triggered`, `entry_triggered`, `exit_triggered`, `decision_intent`,
`signal_intent`, or `signal`; the engine and frontend do not consume them.

System-level risk limits live in `RUNTIME_CONFIG.params` and are interpreted by
backtest and live/simulated runtime through the same field names:

```python
"params": {
    "contract_mode": True,
    "leverage": 3,
    "max_leverage": 5,
    "risk_control_enabled": True,
    "max_single_loss_ratio": 0.02,
    "max_symbol_exposure_pct": 0.20,
    "max_same_direction_exposure_pct": 0.60,
    "max_daily_loss_ratio": 0.05,
    "max_order_value": 2000,
    "require_stop_loss": True,
    "live_fill_sync_symbol_limit": 30,
    "correlation_groups": {
        "major_l1": ["BTC-USDT-SWAP", "ETH-USDT-SWAP", "SOL-USDT-SWAP"],
    },
}
```

`require_stop_loss` rejects entry intents without `stop_loss*` fields or a
matching protective `place_risk_order`. `max_leverage` rejects entries when configured
runtime leverage is above the system limit. `max_symbol_exposure_pct` is the
only strategy parameter name for the per-symbol exposure cap. `max_daily_loss_ratio` blocks new
entries once the current trading day's loss reaches the limit. `max_same_direction_exposure_pct`
caps existing same-side exposure plus the new entry; if `correlation_groups` is
provided, only same-side positions in the same group are counted, otherwise all
same-side positions are treated as one conservative correlated bucket.
`live_fill_sync_symbol_limit` controls how many symbols the live/simulated
runtime covers in each serial REST fill backfill pass; the default is 30 and the
runtime clamps it to `1..50`. It is a coverage limit, not a concurrency setting.

`close_position` is an explicit exit intent, not an entry
action with a special side. Backtest closes the current simulated position for
the action symbol if one exists. Live/simulated runtime is exchange-backed: it
submits the close order to OKX using the current exchange position or spot
balance at submit time. For close actions, `side` can stay `flat`; provide
`order_side`/`close_side` as `buy` or `sell`, or use `side: "long"` to mean
closing a long position with `sell` and `side: "short"` to mean closing a short
position with `buy`. When a live/simulated close action provides
`position_size`, the runtime converts that requested strategy size to the OKX
order quantity and caps the close order at that amount; if `position_size` is
omitted, the close action closes the full currently available exchange
position/balance for the requested side. Planned exits generated from
`planned_exit_time` are tied to the concrete entry exchange order. When the exit
is due, the runtime sums synced OKX fills for that entry order, subtracts fills
already attributed to the planned-exit order, and caps the close order to the
remaining exchange quantity. If the entry fill has not been synced yet, the plan
stays scheduled and retries instead of closing the current total exchange
position. If a planned exit is due but OKX does not yet report a closeable
position, the plan also stays scheduled and retries instead of being terminally
skipped, because exchange fills and position state can arrive later than the
entry order submit acknowledgement. Retried planned exits preserve prior exit
order identities so partial fills from earlier failed/canceled exit attempts are
deducted before submitting the next close order. If the entry order is later
confirmed rejected or canceled without any fill, the runtime cancels the planned
exit. If the entry order is canceled after a partial fill, the planned exit
remains scheduled for the residual exchange position. When multiple
live/simulated entry
orders are submitted from the same strategy action, each exchange entry order
gets its own planned-exit record; a rejected entry only cancels the planned exit
tied to that entry order. Multiple due planned exits on the same symbol and side
are submitted independently so each close is capped by its own entry order's
residual fill quantity. If an already submitted planned-exit close order is
explicitly reported missing by OKX after the submit grace window, the runtime
requeues the planned exit for another close attempt instead of leaving it stuck
in `exit_submitted`.

`place_risk_order` actions are protective order intents. Backtest uses
them as runtime stop/take-profit settings. Live/simulated runtime attaches a
matching protective stop/take-profit algo to the OKX entry order when the same
decision also contains a matching `open_position` action. If the decision only
contains a standalone protective action for a symbol, the runtime resolves the
current OKX closeable position and submits an independent OKX `order-algo`
conditional stop/take-profit order. Standalone protective orders use the current
exchange position as the source of quantity and position side; if
`position_size` is provided, the runtime caps the protective order quantity by
that requested strategy size. Protective trigger prices must already match the
instrument `tickSz`; malformed, unsupported, or off-tick protective orders are
rejected before submitting an unprotected entry or an independent algo order. The
runtime does not silently round protective trigger prices.

`cancel_order` cancels an existing exchange order by `order_id` or
`client_order_id`. If the target local order is a standalone OKX
protective algo order, the runtime uses OKX `cancel-algos`; it can identify the
algo order by either `algoId` in `order_id` or `algoClOrdId` in
`client_order_id`. When the local order state is missing, set
`target_order_kind` to `exchange` or `algo` together with an explicit `symbol`
so the runtime does not guess the OKX endpoint. If `target_order_kind` is omitted
or explicitly set to `any`, the runtime may match either ordinary exchange
orders or protective algo orders; if the same identity matches both, the action
is rejected instead of guessing. Accepted management requests whose original
local order row is missing are persisted as sync candidates for later WS/REST
reconciliation. `modify_order` amends an existing exchange order by the same
identity fields and must provide at least one explicit amend field:
`new_size` or `new_price`. `new_price` must
match the instrument `tickSz`, and `new_size` must match `lotSz` and not be below
`minSz`. Invalid amend values are rejected before OKX submit; the existing order
state is left to exchange WS/REST sync because the original order may still be
live. A generic `price` field is not an amend price; `reference_price` is only a
valuation/display reference, and the live execution layer intentionally ignores
both fields for `modify_order`. If the target local order is a
standalone OKX protective algo order from `place_risk_order`, `modify_order` uses
OKX `amend-algos`; `new_price` is interpreted as the new TP/SL trigger price and
the runtime chooses `newTp*` or `newSl*` fields from the original local
`order_type`. If local state is missing and `target_order_kind` is `algo`,
`modify_order` must also provide `target_order_type` so the runtime can choose
the correct OKX TP/SL amend fields. `target_order_type` is not a normal
exchange `ordType`; it only accepts supported market protective aliases:
`stop_market`, `stop_loss_market`, `sl_market`, `stop_loss`, `stop`,
`take_profit_market`, `tp_market`, or `take_profit`.

For live/simulated entry and close actions with price-required OKX order types
such as `limit`, `post_only`, `fok`, `ioc`, `mmp`, or `mmp_and_post_only`, the
explicit `price` must already match the instrument `tickSz`. The runtime rejects
invalid tick prices before submitting to OKX and records `submit_failed`; it
does not silently round or move the strategy's requested price.

Action dict:

```python
{
    "action": "open_position" | "close_position" | "place_risk_order" | "cancel_order" | "modify_order" | "hold",
    "symbol": "BTC-USDT-SWAP",
    "side": "long" | "short" | "flat" | "buy" | "sell",
    "order_type": "market",
    "reference_price": 123.45,
    "reason": "stable_reason_code",
    "strength": 0.0,
    "position_size": 0.2,
    "timestamp": 1780000000000,
}
```

Model-internal candidate rows may still use feature/source names from the
training dataset. Strategy outputs exposed to the Rust engine should use action
protocol fields; candidate provenance should be reported with fields such as
`source_index`, `source_time`, `feature_bar_time`, `layer_id`, and
`candidate_source`, not old signal visualisation fields.

## Self-Contained Runtime Metadata

Promoted strategies must not require app-side parameter editing. Freeze tuned research values in the strategy file:

```python
RUNTIME_CONFIG = {
    "symbol": "BTC-USDT-SWAP",
    "inst_type": "SWAP",
    "timeframe": "15m",
    "risk_timeframe": "1m",
    "initial_capital": 1000,
    "position_size": 0.2,
    "stop_loss": 0.03,
    "take_profit": 0.06,
    "check_interval": 60,
    "mode": "simulated",
    "params": {
        "contract_mode": True,
        "leverage": 3,
    },
}
```

The runtime discover step rejects strategy files without this metadata. The UI reads it and only needs a strategy selection plus a run action; app-side parameter editing is intentionally not part of the runtime contract.

## Data Requirements

Use `DATA_REQUIREMENTS` to make context collection explicit. The runtime can then decide which K-line sets, funding snapshots, order books, account state, positions, and order state must be loaded before calling `evaluate()`.

```python
DATA_REQUIREMENTS = {
    "candles": [
        {
            "role": "primary",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m",
            "min_bars": 1200,
        }
    ],
    "funding": [],
    "orderbook": [],
    "positions": {"required": False},
    "account": {"required": False},
    "orders": {"open": False, "recent_fills": False, "recent_rejections": False},
}
```

For broad multi-symbol selectors, the shorthand form is also supported:

```python
DATA_REQUIREMENTS = {
    "symbols": ["BTC-USDT-SWAP", "ETH-USDT-SWAP", "SOL-USDT-SWAP"],
    "timeframes": ["1m", "5m", "15m"],
    "funding": True,
    "orderbook": True,
    "account": True,
    "positions": True,
}
```

When `orderbook` is `True`, simulated/live diagnostics and execution fetch the latest order book for each
declared symbol and expose it as `context["orderbook"][symbol]` with normalized
`best_bid`, `best_ask`, `mid_price`, `spread`, `asks`, `bids`, and `ts` fields.
Explicit order book specs are also supported:

```python
DATA_REQUIREMENTS = {
    "orderbook": [
        {"symbol": "BTC-USDT-SWAP", "inst_type": "SWAP", "depth": 5},
        {"symbol": "ETH-USDT-SWAP", "inst_type": "SWAP", "depth": 20, "required": False},
    ],
}
```

Backtest currently rejects required order book context explicitly until a
historical order book loader is available; it no longer silently supplies an
empty order book to strategies that declared the requirement.

When `funding` is `True`, the runtime loads funding history for each declared
symbol and exposes it as:

```python
context["funding"]["BTC-USDT-SWAP"] == {
    "source": "okx_funding_rates",
    "latest": {"funding_time": 1780100000000, "funding_rate": 0.0001, "rate": 0.0001},
    "history": [
        {"funding_time": 1780071200000, "funding_rate": 0.0002, "rate": 0.0002},
        {"funding_time": 1780100000000, "funding_rate": 0.0001, "rate": 0.0001},
    ],
}
```

Funding specs can request a larger lookback or make a symbol optional:

```python
DATA_REQUIREMENTS = {
    "funding": [
        {"symbol": "BTC-USDT-SWAP", "inst_type": "SWAP", "history_limit": 48},
        {"symbol": "ETH-USDT-SWAP", "inst_type": "SWAP", "limit": 12, "required": False},
    ],
}
```

Backtest funding context comes from the local `okx_funding_rates` table and is
trimmed per bar before `evaluate()` sees it, so historical evaluation cannot
read future funding rows. Simulated/live use local `okx_funding_rates` first and can
fall back to the current OKX public funding-rate endpoint for declared symbols.

When a strategy declares stateful inputs, for example `positions: true`,
`account: true`, `orders: true`, or an order bucket such as
`orders.recent_fills: true`, backtest switches from one-shot history evaluation
to per-bar `evaluate()` calls. Each call receives account, position, and order
state produced by the same runtime action executor that is applying previous
actions in the backtest. This is required for add/reduce/close logic, grids,
pair trading, and selectors that must avoid duplicate entries. Simulated/live always
build these sections from current runtime state before each evaluation.

## Visualization Metadata

Use `VISUALIZATION` to document what the strategy wants shown:

```python
VISUALIZATION = {
    "primary_price_series": "close",
    "indicator_series": [
        {"key": "ma_spread", "label": "MA spread", "unit": "ratio", "threshold_key": "spread_threshold"},
    ],
    "diagnostics": ["warmup", "ma_spread", "momentum_return"],
}
```

## Decision Contract

Use `DECISION_CONTRACT` to make strategy outputs parseable by the app:

```python
DECISION_CONTRACT = {
    "action_schema_version": 1,
    "actions": ["open_position", "close_position", "hold"],
    "entry_sides": ["buy", "sell"],
    "exit_sides": ["flat"],
    "hold_sides": ["hold"],
    "reason_codes": ["candidate_breakout_long", "candidate_breakout_short"],
}
```

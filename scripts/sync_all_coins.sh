#!/usr/bin/env bash
# ============================================================
# 批量同步 1m K 线 + 派生所有时间框架。
#
# 根据 OKX 官方文档的限速规则优化：
#   /api/v5/market/history-candles = 20 req / 2 sec / IP
#   → 所有币种共享一个 IP 配额，不能靠币种并行提速
#   → 使用 round-robin: 轮询每个币种拉 1 页(300根)后立即切换
#   → 理论 ~10 req/s → 每个币种 29 页 ~3s → 14 币种 ~42s
#
# 用法:
#   bash scripts/sync_all_coins.sh          # 仅 backfill 1m
#   bash scripts/sync_all_coins.sh --derive # backfill + 派生 3m/5m/15m/1H/4H
# ============================================================
set -euo pipefail
cd "$(dirname "$0")/.."

ROBIN="/tmp/okx_round_robin.py"
DB="data/market.db"

red()   { printf '\033[31m%s\033[0m\n' "$1"; }
green() { printf '\033[32m%s\033[0m\n' "$1"; }
bold()  { printf '\033[1m%s\033[0m' "$1"; }
now()   { date -u +%H:%M:%S; }

# ── 检查 round-robin 脚本是否存在 ──
if [[ ! -f "$ROBIN" ]]; then
  echo "错误: 找不到 $ROBIN，请确保 /tmp/okx_round_robin.py 存在"
  exit 1
fi

# ── backfill ──
echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  策略: Round-robin 轮询 (OKX 20 req/2s 共享 IP 配额)"
echo "  目标: 所有币种从 2025-05-23 到最新"
echo "═══════════════════════════════════════════════════════════"
echo ""

gap_count=$(sqlite3 "$DB" "SELECT COUNT(*) FROM (SELECT 1 FROM candles WHERE timeframe='1m' GROUP BY inst_id HAVING MIN(timestamp) > 1747958400000);")

if [[ "$gap_count" -gt 0 ]]; then
  echo "$(bold ">>> 当前 $gap_count 个币种需要 backfill")"
  echo ""
  python3 "$ROBIN" --db "$DB"
else
  green "所有币种 1m 数据已对齐 ✓"
fi

# ── 检查结果 ──
echo ""
echo "$(bold ">>> 数据状态")"
printf "  %-22s %8s  %-20s → %-20s\n" COIN BARS START END
sqlite3 "$DB" "
  SELECT inst_id, COUNT(*),
         datetime(MIN(timestamp)/1000,'unixepoch'),
         datetime(MAX(timestamp)/1000,'unixepoch')
  FROM candles WHERE timeframe='1m'
  GROUP BY inst_id ORDER BY COUNT(*) DESC;
" -separator " | " | while IFS=" | " read -r coin cnt first last; do
  if [[ "$first" > "2025-05-23" ]]; then
    printf "  $(red '●') %-20s %8s  %-20s → %-20s\n" "$coin" "$cnt" "$first" "$last"
  else
    printf "  $(green '●') %-20s %8s  %-20s → %-20s\n" "$coin" "$cnt" "$first" "$last"
  fi
done

# ── 派生 ──
if [[ "${1:-}" == "--derive" ]]; then
  echo ""
  echo "$(bold ">>> 派生时间框架: 1m → 3m/5m/15m/1H/4H")"
  echo "    (SQLite 窗口函数聚合)"
  echo ""

  all_coins=()
  while IFS= read -r c; do all_coins+=("$c"); done < <(sqlite3 "$DB" "SELECT DISTINCT inst_id FROM candles WHERE timeframe='1m' ORDER BY inst_id;")
  total=${#all_coins[@]}
  done_n=0

  for coin in "${all_coins[@]}"; do
    done_n=$((done_n + 1))
    echo -n "[$(now)] [$done_n/$total] $coin ..."

    sqlite3 -bail "$DB" "BEGIN;
      INSERT OR REPLACE INTO candles (inst_id, inst_type, timeframe, timestamp, open, high, low, close, volume, volume_ccy)
      SELECT inst_id, inst_type, '3m',  (timestamp/180000)*180000,  FIRST_VALUE(open) OVER w, MAX(high) OVER w, MIN(low) OVER w, LAST_VALUE(close) OVER w, SUM(volume) OVER w, SUM(volume_ccy) OVER w
      FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY (timestamp/180000) ORDER BY timestamp) rn FROM candles WHERE inst_id='$coin' AND inst_type='SWAP' AND timeframe='1m') WHERE rn=1;

      INSERT OR REPLACE INTO candles (inst_id, inst_type, timeframe, timestamp, open, high, low, close, volume, volume_ccy)
      SELECT inst_id, inst_type, '5m',  (timestamp/300000)*300000,  FIRST_VALUE(open) OVER w, MAX(high) OVER w, MIN(low) OVER w, LAST_VALUE(close) OVER w, SUM(volume) OVER w, SUM(volume_ccy) OVER w
      FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY (timestamp/300000) ORDER BY timestamp) rn FROM candles WHERE inst_id='$coin' AND inst_type='SWAP' AND timeframe='1m') WHERE rn=1;

      INSERT OR REPLACE INTO candles (inst_id, inst_type, timeframe, timestamp, open, high, low, close, volume, volume_ccy)
      SELECT inst_id, inst_type, '15m', (timestamp/900000)*900000,  FIRST_VALUE(open) OVER w, MAX(high) OVER w, MIN(low) OVER w, LAST_VALUE(close) OVER w, SUM(volume) OVER w, SUM(volume_ccy) OVER w
      FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY (timestamp/900000) ORDER BY timestamp) rn FROM candles WHERE inst_id='$coin' AND inst_type='SWAP' AND timeframe='1m') WHERE rn=1;

      INSERT OR REPLACE INTO candles (inst_id, inst_type, timeframe, timestamp, open, high, low, close, volume, volume_ccy)
      SELECT inst_id, inst_type, '1H',  (timestamp/3600000)*3600000, FIRST_VALUE(open) OVER w, MAX(high) OVER w, MIN(low) OVER w, LAST_VALUE(close) OVER w, SUM(volume) OVER w, SUM(volume_ccy) OVER w
      FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY (timestamp/3600000) ORDER BY timestamp) rn FROM candles WHERE inst_id='$coin' AND inst_type='SWAP' AND timeframe='1m') WHERE rn=1;

      INSERT OR REPLACE INTO candles (inst_id, inst_type, timeframe, timestamp, open, high, low, close, volume, volume_ccy)
      SELECT inst_id, inst_type, '4H',  (timestamp/14400000)*14400000, FIRST_VALUE(open) OVER w, MAX(high) OVER w, MIN(low) OVER w, LAST_VALUE(close) OVER w, SUM(volume) OVER w, SUM(volume_ccy) OVER w
      FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY (timestamp/14400000) ORDER BY timestamp) rn FROM candles WHERE inst_id='$coin' AND inst_type='SWAP' AND timeframe='1m') WHERE rn=1;
    COMMIT;"
    echo " OK"
  done
  green "===== 派生完成 ====="
else
  echo ""
  echo "提示: 加 --derive 可自动派生 3m/5m/15m/1H/4H"
fi

echo ""
green "===== 全部完成 ====="

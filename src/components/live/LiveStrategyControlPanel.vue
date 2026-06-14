<template>
  <div class="vl-control-panel">
    <div class="vl-card">
      <div class="vl-card-header">策略控制</div>
      <div class="vl-card-body">
        <section v-if="formLocked" class="vl-control-section run-summary" aria-labelledby="live-control-running">
          <div class="vl-section-title">
            <span id="live-control-running">当前运行</span>
            <small>停止后才能调整配置</small>
          </div>
          <div class="vl-running-main">
            <strong :title="status?.strategy_name || form.strategy_id || '--'">
              {{ status?.strategy_name || form.strategy_id || '--' }}
            </strong>
            <span>{{ detailDataScope }}</span>
          </div>
        </section>
        <template v-else>
          <section class="vl-control-section primary" aria-labelledby="live-control-config">
            <div class="vl-section-title">
              <span id="live-control-config">运行配置</span>
              <small>启动后按模型输出提交 OKX 订单</small>
            </div>
            <div class="vl-field">
              <label>选择策略</label>
              <ThemeSelect
                :model-value="form.strategy_id"
                :options="strategyOptions"
                placeholder="请选择策略"
                :disabled="actionLoading"
                @update:model-value="emit('updateStrategyId', $event)"
              />
            </div>
            <div class="vl-mode-row" :class="{ live: controlMode === 'live' }">
              <span>运行模式</span>
              <strong>{{ controlModeLabel }}</strong>
            </div>
            <div class="vl-field">
              <label for="live-initial-capital">初始资金</label>
              <input
                id="live-initial-capital"
                name="live-initial-capital"
                class="vl-number-input"
                type="number"
                min="1"
                step="1"
                :disabled="actionLoading"
                :value="form.initial_capital"
                @input="handleInitialCapitalInput"
              >
            </div>
            <div class="vl-capital-preview">
              <span>单笔名义</span>
              <strong>{{ singleOrderNotionalText }}</strong>
            </div>
            <div v-if="riskScopeNote" class="vl-risk-note">{{ riskScopeNote }}</div>
          </section>
        </template>
        <section class="vl-control-section execution" aria-labelledby="live-control-execution">
          <div class="vl-section-title compact">
            <span id="live-control-execution">启动检查</span>
            <small>这里的提示就是启动按钮的真实拦截原因</small>
          </div>
          <div class="vl-launch-state" :class="launchReadiness.kind">
            <span>{{ launchReadiness.title }}</span>
            <strong :title="launchReadiness.detail">{{ launchReadiness.detail }}</strong>
          </div>
          <div class="vl-actions">
            <button
              class="btn start"
              :disabled="Boolean(startDisabledReason)"
              :title="startDisabledReason || '选择参数并启动当前配置'"
              @click="emit('open-run-params')"
            >
              {{ startButtonText }}
            </button>
            <button
              class="btn stop"
              :disabled="Boolean(stopDisabledReason)"
              :title="stopDisabledReason || '停止当前策略'"
              @click="emit('stop')"
            >
              {{ stopButtonText }}
            </button>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { LiveStrategyStatus, TradingMode } from '@/types'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'
import { formatMoney } from '@/utils/format'
import type {
  LiveLaunchReadiness,
} from '@/utils/liveStrategyControlView'
import type { LiveStrategyControlForm } from '@/utils/liveStrategyForm'

type SelectOption = {
  value: string
  label: string
}

const emit = defineEmits<{
  'open-run-params': []
  stop: []
  updateInitialCapital: [value: number]
  updateStrategyId: [value: string]
}>()

const props = defineProps<{
  actionLoading: boolean
  controlMode: TradingMode
  controlModeLabel: string
  detailDataScope: string
  form: LiveStrategyControlForm
  formLocked: boolean
  launchReadiness: LiveLaunchReadiness
  riskScopeNote: string
  startButtonText: string
  startDisabledReason: string
  status: LiveStrategyStatus | null
  stopButtonText: string
  stopDisabledReason: string
  strategyOptions: SelectOption[]
}>()

const singleOrderNotionalText = computed(() => {
  const capital = props.form.initial_capital
  const positionSize = props.form.position_size
  if (!Number.isFinite(capital) || !Number.isFinite(positionSize)) return '--'
  return formatMoney(Math.max(0, capital * positionSize))
})

function handleInitialCapitalInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (!Number.isFinite(value) || value <= 0) return
  emit('updateInitialCapital', value)
}
</script>

<style scoped>
.vl-control-panel {
  min-width: 0;
}
@media (min-width: 1101px) {
  .vl-control-panel .vl-card {
    position: sticky;
    top: 0;
    display: flex;
    max-height: min(760px, calc(100vh - 118px));
    flex-direction: column;
  }

  .vl-control-panel .vl-card-body {
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .vl-control-panel .vl-control-section.execution {
    position: sticky;
    bottom: -12px;
    margin: 10px -12px -12px;
    padding: 10px 12px 12px;
    border-top: 1px solid var(--color-border);
    background: linear-gradient(
      180deg,
      rgba(22, 24, 34, 0.88),
      var(--color-bg-secondary) 28%
    );
    backdrop-filter: blur(4px);
  }
}
.vl-card {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.vl-card-header {
  padding: 8px 12px;
  font-size: 13px;
  font-weight: 600;
  border-bottom: 1px solid var(--color-border);
}
.vl-card-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
}
.vl-control-section {
  min-width: 0;
  padding: 9px;
  border: 1px solid rgba(148,163,184,0.18);
  border-radius: 6px;
  background: rgba(148,163,184,0.035);
}
.vl-control-section.execution {
  background: rgba(15, 17, 23, 0.52);
}
.vl-section-title {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
  min-width: 0;
}
.vl-section-title span {
  flex: 0 0 auto;
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 700;
}
.vl-section-title small {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-tertiary);
  font-size: 10px;
  font-weight: 400;
  line-height: 1.35;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-section-title.compact {
  margin-bottom: 6px;
}
.vl-field { margin-bottom: 8px; }
.vl-control-section .vl-field:last-child {
  margin-bottom: 0;
}
.vl-field label { display: block; font-size: 11px; color: var(--color-text-tertiary); margin-bottom: 2px; }
.vl-number-input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  height: 31px;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.14);
  color: var(--color-text-primary);
  font-size: 12px;
  line-height: 1.35;
  outline: none;
}
.vl-number-input:focus {
  border-color: rgba(41, 98, 255, 0.56);
}
.vl-number-input:disabled {
  cursor: not-allowed;
  opacity: 0.58;
}
.vl-capital-preview {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
  padding: 6px 8px;
  border: 1px solid rgba(148,163,184,0.18);
  border-radius: 4px;
  background: rgba(15, 17, 23, 0.42);
  font-size: 11px;
}
.vl-capital-preview span {
  color: var(--color-text-secondary);
}
.vl-capital-preview strong {
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 700;
}
.vl-mode-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  padding: 6px 8px;
  border: 1px solid rgba(38,166,154,0.28);
  border-radius: 4px;
  background: rgba(38,166,154,0.08);
  font-size: 12px;
}
.vl-mode-row span { color: var(--color-text-secondary); }
.vl-mode-row strong { color: var(--color-positive); font-size: 12px; }
.vl-mode-row.live {
  border-color: rgba(239,83,80,0.32);
  background: rgba(239,83,80,0.08);
}
.vl-mode-row.live strong { color: var(--color-negative); }
.vl-risk-note {
  margin-bottom: 8px;
  padding: 7px 8px;
  border: 1px solid rgba(41, 98, 255, 0.28);
  border-radius: 4px;
  background: rgba(41, 98, 255, 0.08);
  color: var(--color-text-secondary);
  font-size: 11px;
  line-height: 1.45;
}
.vl-running-main {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin-bottom: 8px;
  min-width: 0;
}
.vl-running-main strong,
.vl-running-main span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-running-main strong {
  color: var(--color-text-primary);
  font-size: 13px;
}
.vl-running-main span {
  color: var(--color-text-secondary);
  font-size: 11px;
}
.vl-launch-state {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px;
  border: 1px solid rgba(148,163,184,0.24);
  border-radius: 4px;
  background: rgba(148,163,184,0.06);
  color: var(--color-text-secondary);
  font-size: 12px;
  line-height: 1.45;
}
.vl-launch-state span {
  flex: 0 0 auto;
  color: var(--color-text-primary);
  font-weight: 700;
  white-space: nowrap;
}
.vl-launch-state strong {
  min-width: 0;
  overflow: hidden;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-launch-state.ready {
  border-color: rgba(38,166,154,0.32);
  background: rgba(38,166,154,0.08);
}
.vl-launch-state.ready span { color: var(--color-positive); }
.vl-launch-state.blocked {
  border-color: rgba(239,83,80,0.28);
  background: rgba(239,83,80,0.07);
}
.vl-launch-state.blocked span { color: var(--color-negative); }
.vl-launch-state.locked {
  border-color: rgba(246,200,93,0.3);
  background: rgba(246,200,93,0.08);
}
.vl-launch-state.locked span { color: #f6c85d; }
.vl-launch-state.busy {
  border-color: rgba(41,98,255,0.3);
  background: rgba(41,98,255,0.08);
}
.vl-launch-state.busy span { color: var(--color-accent); }
.vl-actions { display: flex; gap: 6px; margin-top: 12px; }
.btn {
  flex: 1;
  padding: 6px 12px;
  border: none;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.btn.start { background: var(--color-positive); color: #fff; }
.btn.stop { background: var(--color-negative); color: #fff; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }

@media (max-width: 1100px) {
  .vl-section-title {
    flex-direction: column;
    gap: 2px;
  }

  .vl-section-title small {
    text-align: left;
    white-space: normal;
  }

  .vl-launch-state {
    flex-direction: column;
    gap: 4px;
  }

  .vl-launch-state strong {
    white-space: normal;
  }
}
</style>

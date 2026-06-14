<template>
  <div class="journal-editor">
    <div class="je-header">
      <span class="je-title">{{ isEdit ? '编辑日志' : '新建日志' }}</span>
      <button v-if="isEdit" class="cancel-btn" @click="$emit('cancel')">取消</button>
    </div>
    <div class="je-body">
      <div class="je-field">
        <label>标题</label>
        <input v-model="form.title" class="je-input" placeholder="交易日志标题" />
      </div>
      <div class="je-row">
        <div class="je-field">
          <label>品种</label>
          <input v-model="form.inst_id" class="je-input" placeholder="BTC-USDT" />
        </div>
        <div class="je-field">
          <label>交易模式</label>
          <ThemeSelect
            :model-value="form.mode"
            :options="tradingModeOptions"
            @update:model-value="form.mode = $event as TradingMode"
          />
        </div>
        <div class="je-field">
          <label>评级 (1-5)</label>
          <input v-model.number="form.rating" type="number" min="1" max="5" class="je-input" />
        </div>
      </div>
      <div class="je-field">
        <label>标签（逗号分隔）</label>
        <input v-model="tagsInput" class="je-input" placeholder="scalping, breakout" />
      </div>
      <div class="je-field">
        <label>策略</label>
        <input v-model="form.strategy_name" class="je-input" placeholder="策略名称" />
      </div>
      <div class="je-field">
        <label>盈亏快照</label>
        <input v-model.number="form.pnl_snapshot" type="number" step="0.01" class="je-input" />
      </div>
      <div class="je-field">
        <label>内容</label>
        <textarea v-model="form.content" class="je-textarea" rows="6" placeholder="记录交易理由、执行情况、经验教训..."></textarea>
      </div>
      <button class="submit-btn" @click="submit" :disabled="!form.title || submitting">
        {{ submitting ? '保存中...' : '保存' }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { JournalEntry, TradingMode } from '@/types'
import { useJournalEditor } from '@/composables/useJournalEditor'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'

const props = defineProps<{ entry: JournalEntry | null }>()
const emit = defineEmits<{
  save: [data: Partial<JournalEntry>]
  cancel: []
}>()

const {
  isEdit,
  submitting,
  tradingModeOptions,
  form,
  tagsInput,
  submit,
} = useJournalEditor(props, {
  onSave: data => emit('save', data),
})
</script>

<style scoped>
.journal-editor {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.je-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.je-title { font-size: 13px; font-weight: 600; }
.cancel-btn {
  padding: 2px 10px;
  border: 1px solid var(--color-border);
  border-radius: 3px;
  background: none;
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.je-body { padding: 10px; display: flex; flex-direction: column; gap: 8px; }
.je-row { display: flex; gap: 8px; }
.je-row .je-field { flex: 1; }
.je-field { display: flex; flex-direction: column; gap: 2px; }
.je-field label { font-size: 11px; color: var(--color-text-tertiary); }
.je-input {
  padding: 5px 8px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-primary);
  font-size: 12px;
  outline: none;
}
.je-input:focus { border-color: var(--color-accent); }
.je-textarea {
  padding: 6px 8px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-primary);
  font-size: 12px;
  outline: none;
  resize: vertical;
  font-family: inherit;
}
.je-textarea:focus { border-color: var(--color-accent); }
.submit-btn {
  padding: 7px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.submit-btn:disabled { opacity: 0.4; cursor: not-allowed; }
</style>

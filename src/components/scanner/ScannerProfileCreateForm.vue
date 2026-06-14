<template>
  <div class="sp-create">
    <input v-model="form.name" class="sp-input" placeholder="配置名称" />
    <div class="sp-conds">
      <label v-for="condition in conditions" :key="condition.id" class="sp-cond-label">
        <input v-model="form.conditions" type="checkbox" :value="condition.id" />
        {{ condition.name }}
      </label>
    </div>
    <div class="sp-actions">
      <button class="btn" @click="handleCreate">创建</button>
      <button class="btn secondary" @click="$emit('cancel')">取消</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { ScannerCondition } from '@/types'

defineProps<{ conditions: ScannerCondition[] }>()
const emit = defineEmits<{
  create: [data: { name: string; conditions: string[] }]
  cancel: []
}>()

const form = ref({ name: '', conditions: [] as string[] })

function handleCreate() {
  const name = form.value.name.trim()
  if (!name) return
  emit('create', { name, conditions: [...form.value.conditions] })
  form.value = { name: '', conditions: [] }
}
</script>

<style scoped>
.sp-create { padding: 10px 12px; border-bottom: 1px solid var(--color-border); }
.sp-input {
  width: 100%;
  padding: 6px 8px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-primary);
  font-size: 12px;
  margin-bottom: 8px;
}
.sp-conds { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 8px; }
.sp-cond-label { font-size: 11px; display: flex; align-items: center; gap: 3px; }
.sp-actions { display: flex; gap: 6px; }
.btn {
  padding: 4px 12px;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  background: var(--color-accent);
  color: #fff;
}
.btn.secondary { background: var(--color-bg-primary); color: var(--color-text-secondary); }
</style>

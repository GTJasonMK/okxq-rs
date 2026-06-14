<template>
  <div class="chat-input">
    <textarea
      v-model="text"
      class="ci-textarea"
      :placeholder="placeholder"
      rows="2"
      @keydown.enter.exact.prevent="send"
      @keydown.shift.enter.exact="() => {}"
    ></textarea>
    <button class="ci-send" @click="send" :disabled="!text.trim() || disabled">
      {{ disabled ? '发送中...' : '发送' }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const props = withDefaults(defineProps<{ disabled?: boolean; placeholder?: string }>(), {
  disabled: false,
  placeholder: '输入消息... (Enter 发送, Shift+Enter 换行)',
})

const emit = defineEmits<{ send: [text: string] }>()

const text = ref('')

function send() {
  const t = text.value.trim()
  if (!t || props.disabled) return
  emit('send', t)
  text.value = ''
}
</script>

<style scoped>
.chat-input {
  display: flex;
  gap: 8px;
  align-items: flex-end;
  padding: 8px;
  border-top: 1px solid var(--color-border);
  background: var(--color-bg-secondary);
}
.ci-textarea {
  flex: 1;
  padding: 6px 10px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-primary);
  font-size: 12px;
  resize: vertical;
  max-height: 100px;
  line-height: 1.4;
  font-family: inherit;
}
.ci-send {
  padding: 6px 14px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
}
.ci-send:disabled { opacity: 0.5; cursor: not-allowed; }
</style>

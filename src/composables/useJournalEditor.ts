import { computed, reactive, ref, watch } from 'vue'
import type { JournalEntry, TradingMode } from '@/types'

interface JournalEditorProps {
  entry: JournalEntry | null
}

interface UseJournalEditorOptions {
  onSave?: (data: Partial<JournalEntry>) => void
}

const tradingModeOptions = [
  { value: 'live' as TradingMode, label: '实盘' },
  { value: 'simulated' as TradingMode, label: '模拟' },
]

export function useJournalEditor(
  props: JournalEditorProps,
  options: UseJournalEditorOptions = {},
) {
  const isEdit = computed(() => !!props.entry)
  const submitting = ref(false)

  const form = reactive({
    title: '',
    content: '',
    inst_id: '',
    mode: 'live' as TradingMode,
    rating: 3,
    strategy_name: '',
    pnl_snapshot: 0,
  })

  const tagsInput = ref('')

  function fillFrom(entry: JournalEntry) {
    form.title = entry.title || ''
    form.content = entry.content || ''
    form.inst_id = entry.inst_id || ''
    form.mode = entry.mode || 'simulated'
    form.rating = entry.rating || 3
    form.strategy_name = entry.strategy_name || ''
    form.pnl_snapshot = entry.pnl_snapshot || 0
    tagsInput.value = (entry.tags || []).join(', ')
  }

  function resetForm() {
    form.title = ''
    form.content = ''
    form.inst_id = ''
    form.mode = 'simulated'
    form.rating = 3
    form.strategy_name = ''
    form.pnl_snapshot = 0
    tagsInput.value = ''
  }

  watch(() => props.entry, (entry) => {
    if (entry) fillFrom(entry)
    else resetForm()
  }, { immediate: true })

  function submit() {
    submitting.value = true
    const tags = tagsInput.value.split(',').map(tag => tag.trim()).filter(Boolean)
    options.onSave?.({ ...form, tags })
    submitting.value = false
  }

  return {
    isEdit,
    submitting,
    tradingModeOptions,
    form,
    tagsInput,
    submit,
  }
}

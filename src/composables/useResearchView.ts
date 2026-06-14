import { onMounted, ref } from 'vue'
import * as api from '@/api/research'
import { describeError } from '@/utils/logger'
import { settledErrorMessage } from '@/utils/settled'

const LOAD_LABELS = ['数据集', '训练记录']

type ResearchDatasetListItem = {
  id: string
  name?: string
  created_at?: string
}

export function useResearchView() {
  const datasets = ref<ResearchDatasetListItem[]>([])
  const runs = ref<Array<Record<string, unknown>>>([])
  const activeDatasetId = ref<string | null>(null)
  const activeRun = ref<Record<string, unknown> | null>(null)
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)

  async function loadData() {
    error.value = null
    const [datasetsResult, runsResult] = await Promise.allSettled([
      api.fetchDatasets(),
      api.fetchTrainingRuns(),
    ])
    if (datasetsResult.status === 'fulfilled') datasets.value = datasetsResult.value as never
    if (runsResult.status === 'fulfilled') runs.value = runsResult.value as never
    error.value = settledErrorMessage([datasetsResult, runsResult], LOAD_LABELS)
  }

  function selectDataset(dataset: Record<string, unknown>) {
    activeDatasetId.value = dataset.id as string
  }

  async function buildDataset(params: Record<string, unknown>) {
    error.value = null
    message.value = null
    try {
      const dataset = await api.buildDataset(params.inst_id as string, params.bar_count as number, {
        inst_type: params.inst_type as never,
        timeframe: params.timeframe as string,
      })
      activeDatasetId.value = String(dataset.id || '')
      message.value = '数据集已构建'
      await loadData()
    } catch (e) {
      error.value = describeError(e)
    }
  }

  async function trainModel() {
    if (!activeDatasetId.value) return
    error.value = null
    message.value = null
    try {
      const run = await api.trainModel(activeDatasetId.value)
      activeRun.value = run as Record<string, unknown>
      message.value = '模型训练完成'
      await loadData()
    } catch (e) {
      error.value = describeError(e)
    }
  }

  function selectRun(run: Record<string, unknown>) {
    activeRun.value = run
  }

  onMounted(() => {
    void loadData()
  })

  return {
    datasets,
    runs,
    activeDatasetId,
    activeRun,
    error,
    message,
    selectDataset,
    buildDataset,
    trainModel,
    selectRun,
  }
}

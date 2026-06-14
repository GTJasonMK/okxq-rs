import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useScannerStore = defineStore('scanner', () => {
  const profiles = ref<unknown[]>([])
  const results = ref<unknown[]>([])
  const conditions = ref<unknown[]>([])
  const loading = ref(false)
  return { profiles, results, conditions, loading }
})

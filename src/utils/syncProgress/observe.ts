const SYNC_JOB_OBSERVE_BATCH_SIZE = 200

export function nextObservedTaskBatch(
  pending: Set<string>,
  batchSize = SYNC_JOB_OBSERVE_BATCH_SIZE,
) {
  const batch: string[] = []
  for (const taskId of pending) {
    batch.push(taskId)
    if (batch.length >= batchSize) break
  }
  return batch
}

export function rotateObservedTaskBatch(pending: Set<string>, batchTaskIds: string[]) {
  for (const taskId of batchTaskIds) {
    if (!pending.has(taskId)) continue
    pending.delete(taskId)
    pending.add(taskId)
  }
}

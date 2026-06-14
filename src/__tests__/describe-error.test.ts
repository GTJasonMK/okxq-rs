import { describe, expect, it } from 'vitest'
import { describeError } from '@/utils/logger'

describe('错误信息归一化', () => {
  it('把 Tauri 命令未加载错误转换为可操作提示', () => {
    expect(describeError('Command local_api_request not found')).toContain(
      '执行 npm run dev:cleanup 后再用 npm run dev 启动完整桌面端',
    )
  })
})

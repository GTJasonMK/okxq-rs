import { describe, expect, it } from 'vitest'
import capability from '../../src-tauri/capabilities/default.json?raw'
import permissions from '../../src-tauri/permissions/app-commands.toml?raw'
import libRs from '../../src-tauri/src/lib.rs?raw'

describe('Tauri 应用命令 ACL', () => {
  it('显式授权所有注册到 invoke_handler 的应用命令', () => {
    const registeredCommands = extractGenerateHandlerCommands(libRs)
    const allowedCommands = extractAllowedCommands(permissions)

    expect(capability).toContain('"allow-app-commands"')
    expect(registeredCommands).toEqual(expect.arrayContaining(['local_api_request']))
    expect(allowedCommands).toEqual(expect.arrayContaining(registeredCommands))
  })
})

function extractGenerateHandlerCommands(source: string): string[] {
  const match = source.match(/generate_handler!\[\s*([\s\S]*?)\s*\]/)
  if (!match) return []
  return match[1]
    .split(',')
    .map(command => command.trim())
    .filter(command => /^[a-zA-Z_][a-zA-Z0-9_]*$/.test(command))
}

function extractAllowedCommands(source: string): string[] {
  const match = source.match(/commands\.allow\s*=\s*\[([\s\S]*?)\]/)
  if (!match) return []
  return [...match[1].matchAll(/"([^"]+)"/g)].map(item => item[1])
}

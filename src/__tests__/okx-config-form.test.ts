import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import OkxConfigForm from '@/components/settings/OkxConfigForm.vue'
import type { OkxConfig } from '@/types/system'

describe('OkxConfigForm', () => {
  it('展示当前模式、代理和已脱敏凭证', () => {
    const wrapper = mount(OkxConfigForm, {
      props: {
        config: okxConfig(),
      },
    })

    expect(wrapper.get('.oc-current').text()).toBe('当前：实盘')
    expect(wrapper.get<HTMLInputElement>('#okx-proxy-url').element.value).toBe(' http://127.0.0.1:7897 ')
    expect(wrapper.get('#okx-demo-api-key').attributes('placeholder')).toBe('留空保留当前值')
    expect(wrapper.get('#okx-live-secret-key').attributes('placeholder')).toBe('留空保留当前值')
    expect(wrapper.text()).toContain('当前 ***demo-key')
    expect(wrapper.text()).toContain('当前 ***live-secret')
  })

  it('保存时发送当前草稿并只裁剪代理地址', async () => {
    const wrapper = mount(OkxConfigForm, {
      props: {
        config: okxConfig(),
      },
    })

    await wrapper.findAll('input[type="radio"]')[0].trigger('change')
    await wrapper.get('#okx-proxy-url').setValue(' http://127.0.0.1:7898 ')
    await wrapper.get('#okx-demo-api-key').setValue(' demo-key ')
    await wrapper.get('#okx-demo-secret-key').setValue(' demo-secret ')
    await wrapper.get('#okx-demo-passphrase').setValue(' demo-pass ')
    await wrapper.get('#okx-live-api-key').setValue(' live-key ')
    await wrapper.get('#okx-live-secret-key').setValue(' live-secret ')
    await wrapper.get('#okx-live-passphrase').setValue(' live-pass ')
    await wrapper.get('.save-btn').trigger('click')

    expect(wrapper.emitted('save')?.[0]).toEqual([{
      use_simulated: true,
      proxy_url: 'http://127.0.0.1:7898',
      demo: {
        api_key: ' demo-key ',
        secret_key: ' demo-secret ',
        passphrase: ' demo-pass ',
      },
      live: {
        api_key: ' live-key ',
        secret_key: ' live-secret ',
        passphrase: ' live-pass ',
      },
    }])
  })

  it('测试配置复用同一份当前草稿 payload', async () => {
    const wrapper = mount(OkxConfigForm, {
      props: {
        config: okxConfig(),
      },
    })

    await wrapper.findAll('input[type="radio"]')[0].trigger('change')
    await wrapper.get('#okx-demo-api-key').setValue('demo-key')
    await wrapper.get('.test-btn').trigger('click')

    expect(wrapper.emitted('test')?.[0]?.[0]).toMatchObject({
      use_simulated: true,
      demo: {
        api_key: 'demo-key',
      },
    })
  })
})

function okxConfig(): OkxConfig {
  return {
    use_simulated: false,
    is_configured: true,
    proxy_url: ' http://127.0.0.1:7897 ',
    effective_proxy_url: 'DIRECT',
    demo: {
      api_key: '***demo-key',
      secret_key: '***demo-secret',
      passphrase: '***demo-pass',
      is_configured: true,
    },
    live: {
      api_key: '***live-key',
      secret_key: '***live-secret',
      passphrase: '***live-pass',
      is_configured: true,
    },
  }
}

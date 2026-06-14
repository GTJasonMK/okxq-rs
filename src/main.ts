import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import { installGlobalLogging, logger } from '@/utils/logger'

const app = createApp(App)

installGlobalLogging()

app.config.errorHandler = (error, instance, info) => {
  logger.error('vue error', {
    scope: 'vue',
    info,
    component: instance?.$options.name,
    error,
  })
}

if (import.meta.env.DEV) {
  app.config.warnHandler = (message, instance, trace) => {
    logger.warn('vue warning', {
      scope: 'vue',
      message,
      component: instance?.$options.name,
      trace,
    })
  }
}

app.use(createPinia())
app.use(router)
app.mount('#app')

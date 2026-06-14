<template>
  <AppShell />
</template>

<script setup lang="ts">
import AppShell from '@/components/layout/AppShell.vue'
</script>

<style>
:root {
  --color-bg-primary: #0f1117;
  --color-bg-secondary: #161822;
  --color-bg-sidebar: #12141c;
  --color-bg-hover: rgba(255, 255, 255, 0.06);
  --color-bg-active: rgba(41, 98, 255, 0.12);
  --color-border: #1e2030;
  --color-text-primary: #e0e0e0;
  --color-text-secondary: #8888a0;
  --color-text-tertiary: #5a5a72;
  --color-accent: #2962ff;
  --color-positive: #26a69a;
  --color-negative: #ef5350;
  --color-warning: #f5a623;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  height: 100%;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
    'Helvetica Neue', Arial, sans-serif;
  font-size: 14px;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  -webkit-font-smoothing: antialiased;
}

#app {
  height: 100%;
}

.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.neutral { color: var(--color-text-secondary); }

::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
::-webkit-scrollbar-thumb {
  background: #2a2a3a;
  border-radius: 3px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
</style>

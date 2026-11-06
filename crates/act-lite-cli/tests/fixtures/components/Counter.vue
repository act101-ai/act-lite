<!-- Zähler: ein Zähler -->
<script lang="ts">
export default { name: 'Counter' }
</script>

<script setup lang="ts">
import { ref } from 'vue'
const props = defineProps<{ start: number }>()
const count = ref(props.start)
function increment(step: number): void {
  if (step > 0 && count.value < 10) {
    count.value += step
  }
}
</script>

<template>
  <div id="app" class="counter big">
    <button @click="increment(1)">+</button>
    <span v-if="count > 5">{{ count }}</span>
  </div>
</template>

<style scoped>
.counter { color: red; }
</style>

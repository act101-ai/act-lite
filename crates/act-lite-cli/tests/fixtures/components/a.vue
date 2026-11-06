<template>
  <p class="greeting">{{ message }}</p>
</template>

<script setup lang="ts">
import { format } from "./format";

const message = format("hello");
</script>

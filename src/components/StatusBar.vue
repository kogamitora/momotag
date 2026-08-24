<script setup lang="ts">
import { AlertTriangle, CheckCircle2, LoaderCircle } from "lucide-vue-next";

defineProps<{
  kind: "idle" | "info" | "success" | "error";
  message: string;
}>();
</script>

<template>
  <div
    v-if="message"
    class="mt-auto flex items-center gap-2 border-t border-[#d7e0e5] px-5 py-3.5"
    :class="{
      'text-[#1d6a3b]': kind === 'success',
      'text-danger': kind === 'error',
      'text-accent-dark': kind === 'info',
    }"
  >
    <AlertTriangle v-if="kind === 'error'" :size="18" aria-hidden="true" />
    <CheckCircle2 v-else-if="kind === 'success'" :size="18" aria-hidden="true" />
    <LoaderCircle v-else-if="kind === 'info'" class="spin" :size="18" aria-hidden="true" />
    <span>{{ message }}</span>
  </div>
</template>

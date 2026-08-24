<script setup lang="ts">
import { ImagePlus } from "lucide-vue-next";

defineProps<{
  visible: boolean;
  coverPreview: string;
  title: string;
  meta: string;
  previewTitle: string;
  coverAlt: string;
}>();

const emit = defineEmits<{ (event: "open-cover"): void }>();
</script>

<template>
  <section v-if="visible" class="grid grid-cols-[86px_minmax(0,1fr)] items-center gap-3.5 border-b border-[#d7e0e5] px-5 py-4">
    <button
      v-if="coverPreview"
      class="flex aspect-square w-21.5 cursor-zoom-in items-center justify-center overflow-hidden rounded-lg border border-[#c7d9dc] bg-[#e6f2f1] p-0 text-accent-dark transition duration-200 hover:-translate-y-px hover:border-accent focus-visible:border-accent focus-visible:outline-3 focus-visible:outline-accent/20 focus-visible:outline-offset-2"
      type="button"
      :title="previewTitle"
      @click="emit('open-cover')"
    >
      <img class="h-full w-full object-cover transition duration-200 hover:scale-[1.04]" :src="coverPreview" :alt="coverAlt" />
    </button>
    <div v-else class="flex aspect-square w-21.5 items-center justify-center overflow-hidden rounded-lg border border-[#c7d9dc] bg-[#e6f2f1] text-accent-dark">
      <ImagePlus :size="28" aria-hidden="true" />
    </div>
    <div>
      <h3 v-if="title.trim()" class="wrap-break-word text-lg font-bold leading-tight">{{ title }}</h3>
      <p v-if="meta" class="mt-1.5 wrap-break-word text-[13px] leading-[1.35] text-muted">{{ meta }}</p>
    </div>
  </section>
</template>

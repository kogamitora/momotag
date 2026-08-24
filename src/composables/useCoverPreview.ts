import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { chooseCoverImage, loadCoverPreview } from "../api/musicTags";

export function useCoverPreview(onError: (error: unknown) => void) {
  const coverPath = ref("");
  const coverPreview = ref("");
  const isCoverPreviewOpen = ref(false);

  watch(coverPreview, (preview) => {
    if (!preview) isCoverPreviewOpen.value = false;
  });

  onMounted(() => window.addEventListener("keydown", handleWindowKeydown));
  onBeforeUnmount(() => {
    window.removeEventListener("keydown", handleWindowKeydown);
    revokePreview();
  });

  async function pickCover() {
    const selected = await chooseCoverImage();
    if (!selected) return;

    coverPath.value = selected;
    await refreshCoverPreview(selected);
  }

  async function handleCoverSourceChange() {
    const source = coverPath.value.trim();
    if (!source) {
      revokePreview();
      coverPreview.value = "";
      return;
    }

    await refreshCoverPreview(source);
  }

  async function refreshCoverPreview(path: string) {
    try {
      const image = await loadCoverPreview(path);
      revokePreview();
      coverPreview.value = URL.createObjectURL(
        new Blob([new Uint8Array(image.data)], { type: image.mimeType }),
      );
    } catch (error) {
      revokePreview();
      coverPreview.value = "";
      onError(error);
    }
  }

  function openCoverPreview() {
    if (coverPreview.value) isCoverPreviewOpen.value = true;
  }

  function closeCoverPreview() {
    isCoverPreviewOpen.value = false;
  }

  function resetCoverPreview() {
    revokePreview();
    coverPath.value = "";
    coverPreview.value = "";
    isCoverPreviewOpen.value = false;
  }

  function revokePreview() {
    if (coverPreview.value) URL.revokeObjectURL(coverPreview.value);
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") closeCoverPreview();
  }

  return {
    coverPath,
    coverPreview,
    isCoverPreviewOpen,
    pickCover,
    handleCoverSourceChange,
    openCoverPreview,
    closeCoverPreview,
    resetCoverPreview,
  };
}

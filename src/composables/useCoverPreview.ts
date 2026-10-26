import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { chooseCoverImage, loadCoverPreview } from "../api/musicTags";

export function useCoverPreview(onError: (error: unknown) => void) {
  const coverPath = ref("");
  const coverPreview = ref("");
  const isCoverPreviewOpen = ref(false);
  const isCoverLoading = ref(false);
  let coverRequestId = 0;
  let lastHandledSource = "";
  let loadingSource = "";

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
    await handleCoverSourceChange();
  }

  async function handleCoverSourceChange() {
    const source = coverPath.value.trim();
    if (source === lastHandledSource) return;

    if (!source) {
      coverRequestId += 1;
      lastHandledSource = "";
      loadingSource = "";
      isCoverLoading.value = false;
      revokePreview();
      coverPreview.value = "";
      return;
    }

    lastHandledSource = source;
    if (source === loadingSource) return;
    await refreshCoverPreview(source);
  }

  async function refreshCoverPreview(path: string) {
    const currentId = ++coverRequestId;
    loadingSource = path;
    isCoverLoading.value = true;
    try {
      if (isRemoteImageSource(path)) {
        if (currentId !== coverRequestId) return;
        revokePreview();
        coverPreview.value = path;
        return;
      }

      const image = await loadCoverPreview(path);
      if (currentId !== coverRequestId) return;
      revokePreview();
      coverPreview.value = URL.createObjectURL(
        new Blob([new Uint8Array(image.data)], { type: image.mimeType }),
      );
    } catch (error) {
      if (currentId !== coverRequestId) return;
      revokePreview();
      coverPreview.value = "";
      // Keep failed sources retryable: a later change/blur/Enter should retry
      // the same URL or path instead of being treated as already handled.
      lastHandledSource = "";
      onError(error);
    } finally {
      if (currentId === coverRequestId) {
        loadingSource = "";
        isCoverLoading.value = false;
      }
    }
  }

  function openCoverPreview() {
    if (coverPreview.value) isCoverPreviewOpen.value = true;
  }

  function closeCoverPreview() {
    isCoverPreviewOpen.value = false;
  }

  function resetCoverPreview() {
    coverRequestId += 1;
    lastHandledSource = "";
    loadingSource = "";
    isCoverLoading.value = false;
    revokePreview();
    coverPath.value = "";
    coverPreview.value = "";
    isCoverPreviewOpen.value = false;
  }

  function revokePreview() {
    if (coverPreview.value.startsWith("blob:")) URL.revokeObjectURL(coverPreview.value);
  }

  function isRemoteImageSource(source: string) {
    return /^https?:\/\//i.test(source);
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") closeCoverPreview();
  }

  return {
    coverPath,
    coverPreview,
    isCoverPreviewOpen,
    isCoverLoading,
    pickCover,
    handleCoverSourceChange,
    openCoverPreview,
    closeCoverPreview,
    resetCoverPreview,
  };
}

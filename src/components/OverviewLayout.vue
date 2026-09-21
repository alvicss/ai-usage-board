<!--
  檔案：components/OverviewLayout.vue
  用途：並排排版（FR-4）：啟用的各家同時顯示；視窗寬高比 ≥ 1.6 時橫排，否則直排（方向由 utils/layout.ts 算出）。
        右上角三顆按鈕落在「最右上那張卡片」的標題列：橫排是最後一張，直排是第一張。
  誰會用到：WidgetView。
-->
<script setup lang="ts">
import type { ColorScheme, Settings } from "../types/settings";
import type { ProviderUsage } from "../types/usage";
import type { WidgetLayout } from "../utils/layout";
import ProviderCard from "./ProviderCard.vue";

const props = defineProps<{
  providers: ProviderUsage[];
  settings: Settings;
  scheme: ColorScheme;
  now: number;
  layout: WidgetLayout;
}>();

/** 這張卡片的標題列是否要留位置給右上角按鈕 */
function reservesToolbar(index: number): boolean {
  if (props.layout.direction === "row") {
    return index === props.providers.length - 1;
  }
  return index === 0;
}
</script>

<template>
  <section class="overview" :data-direction="layout.direction">
    <ProviderCard
      v-for="(provider, index) in providers"
      :key="provider.id"
      :provider="provider"
      :settings="settings"
      :scheme="scheme"
      :now="now"
      :scale="layout.scale"
      :card-width="layout.cardWidth"
      :card-height="layout.cardHeight"
      :force-compact="layout.forceCompact"
      :reserve-toolbar="reservesToolbar(index)"
    />
  </section>
</template>

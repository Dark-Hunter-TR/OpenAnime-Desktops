<script lang="ts">
  import { slide } from "svelte/transition";

  let { title, isOpen = $bindable(false), children } = $props<{
    title: string;
    isOpen?: boolean;
    children: import('svelte').Snippet;
  }>();

  function toggle() {
    isOpen = !isOpen;
  }
</script>

<div class="accordion-item" class:open={isOpen}>
  <button class="accordion-header" onclick={toggle} aria-expanded={isOpen}>
    <span class="accordion-title">{title}</span>
    <span class="accordion-arrow">{isOpen ? "▲" : "▼"}</span>
  </button>
  
  {#if isOpen}
    <div class="accordion-content" transition:slide={{ duration: 150 }}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .accordion-item {
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.02);
    border-radius: 8px;
    margin-bottom: 8px;
    overflow: hidden;
    transition: border-color 0.15s;
  }
  .accordion-item:hover {
    border-color: rgba(255, 255, 255, 0.08);
  }
  .accordion-item.open {
    border-color: rgba(255, 255, 255, 0.1);
  }
  .accordion-header {
    width: 100%;
    background: transparent;
    border: none;
    padding: 14px 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary, #e4e7ec);
    user-select: none;
  }
  .accordion-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-secondary, #9ba3b4);
  }
  .accordion-arrow {
    font-size: 10px;
    color: var(--text-disabled, #5c6478);
    transition: transform 0.15s ease;
  }
  .accordion-content {
    padding: 0 16px 16px;
  }
</style>

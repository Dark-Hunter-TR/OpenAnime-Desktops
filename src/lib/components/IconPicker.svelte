<script lang="ts">
  import { fluentIcons, type FluentIcon } from "../icons/fluent";

  let { 
    isOpen = $bindable(false), 
    onSelect, 
    onClose 
  } = $props<{
    isOpen: boolean;
    onSelect: (path: string) => void;
    onClose?: () => void;
  }>();

  let searchQuery = $state("");
  let activeCategory = $state<string>("All");

  const categories = ["All", "Navigation", "Media", "Action", "Status", "User"];

  const filteredIcons = $derived.by(() => {
    return fluentIcons.filter(icon => {
      const matchesSearch = icon.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
                            icon.id.toLowerCase().includes(searchQuery.toLowerCase());
      const matchesCategory = activeCategory === "All" || icon.category === activeCategory;
      return matchesSearch && matchesCategory;
    });
  });

  const getIcon = (id: string) => fluentIcons.find(i => i.id === id)?.path || "";

  function handleSelect(path: string) {
    onSelect(path);
    isOpen = false;
  }

  function handleClose() {
    isOpen = false;
    if (onClose) onClose();
  }
</script>

{#snippet icon(id: string, size = 14)}
  <svg viewBox="0 0 24 24" width={size} height={size} class="inline-svg-icon" fill="currentColor">
    {@html getIcon(id)}
  </svg>
{/snippet}

{#if isOpen}
  <div class="modal-backdrop" onclick={handleClose} role="presentation">
    <div 
      class="modal-container" 
      onclick={e => e.stopPropagation()} 
      onkeydown={e => e.stopPropagation()} 
      role="dialog" 
      aria-modal="true" 
      aria-labelledby="modal-title"
      tabindex="-1"
    >
      <!-- Header -->
      <header class="modal-header">
        <h3 id="modal-title">{@render icon('sparkles_regular', 18)} Fluent İkon Seçici</h3>
        <button class="close-btn" onclick={handleClose} aria-label="Kapat">
          {@render icon('dismiss_regular', 16)}
        </button>
      </header>

      <!-- Search & Filters -->
      <div class="modal-filters">
        <div class="search-box">
          <span class="search-icon">
            {@render icon('search_regular', 14)}
          </span>
          <input 
            type="text" 
            bind:value={searchQuery} 
            placeholder="İkon ara..." 
            spellcheck="false" 
          />
          {#if searchQuery}
            <button class="clear-btn" onclick={() => searchQuery = ""}>
              {@render icon('dismiss_regular', 12)}
            </button>
          {/if}
        </div>

        <nav class="categories-tabs">
          {#each categories as category}
            <button 
              class="category-btn" 
              class:active={activeCategory === category}
              onclick={() => activeCategory = category}
            >
              {category === "All" ? "Hepsi" : category}
            </button>
          {/each}
        </nav>
      </div>

      <!-- Grid -->
      <div class="icons-scroll">
        {#if filteredIcons.length === 0}
          <div class="empty-state">
            <span class="empty-icon">{@render icon('search_regular', 32)}</span>
            <p>Aradığınız kriterlere uygun ikon bulunamadı.</p>
          </div>
        {:else}
          <div class="icons-grid">
            {#each filteredIcons as icon}
              <button 
                class="icon-card" 
                onclick={() => handleSelect(icon.path)}
                title={icon.name}
              >
                <div class="icon-preview">
                  <svg viewBox="0 0 24 24" width="24" height="24">
                    {@html icon.path}
                  </svg>
                </div>
                <span class="icon-label">{icon.name.split(' (')[0]}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(10, 12, 18, 0.75);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 99999;
    animation: fadeIn 0.18s ease-out;
  }

  .modal-container {
    width: 600px;
    max-width: 90vw;
    height: 500px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: slideIn 0.22s cubic-bezier(0.1, 0.9, 0.2, 1);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 18px 24px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 16px;
    padding: 4px;
    border-radius: 4px;
    transition: all 0.12s;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-surface-hover);
  }

  .modal-filters {
    padding: 16px 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: rgba(0, 0, 0, 0.1);
    border-bottom: 1px solid var(--border);
  }

  .search-box {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 12px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .search-box input {
    width: 100%;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px 10px 36px;
    color: var(--text-primary);
    font-size: 13.5px;
    outline: none;
    transition: all 0.12s;
  }

  .search-box input:focus {
    border-color: var(--accent);
    background: var(--bg-surface-hover);
    box-shadow: 0 0 0 1px rgba(88, 101, 242, 0.25);
  }

  .clear-btn {
    position: absolute;
    right: 12px;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
  }
  .clear-btn:hover {
    color: var(--text-primary);
  }

  .categories-tabs {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    scrollbar-width: none; /* Firefox */
  }

  .categories-tabs::-webkit-scrollbar {
    display: none; /* Safari and Chrome */
  }

  .category-btn {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-secondary);
    padding: 6px 12px;
    border-radius: 20px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.12s;
  }

  .category-btn:hover {
    color: var(--text-primary);
    background: var(--bg-surface-hover);
  }

  .category-btn.active {
    background: var(--bg-surface-hover);
    border-color: var(--accent);
    color: var(--accent);
  }

  .icons-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }

  .icons-scroll::-webkit-scrollbar {
    width: 6px;
  }
  .icons-scroll::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb);
    border-radius: 3px;
  }

  .icons-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(88px, 1fr));
    gap: 12px;
  }

  .icon-card {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    transition: all 0.15s cubic-bezier(0.1, 0.9, 0.2, 1);
  }

  .icon-card:hover {
    background: var(--bg-surface-hover);
    border-color: var(--accent);
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }

  .icon-preview {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.15);
    border-radius: 8px;
    color: var(--text-secondary);
    transition: color 0.12s;
  }

  .icon-card:hover .icon-preview {
    color: var(--accent);
  }

  .icon-label {
    font-size: 11px;
    color: var(--text-secondary);
    text-align: center;
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .icon-card:hover .icon-label {
    color: var(--text-primary);
  }

  .empty-state {
    text-align: center;
    padding: 40px 0;
    color: var(--text-secondary);
  }

  .empty-icon {
    font-size: 32px;
    margin-bottom: 8px;
    display: block;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slideIn {
    from { 
      opacity: 0;
      transform: scale(0.96) translateY(8px);
    }
    to { 
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .inline-svg-icon {
    display: inline-flex;
    align-self: center;
    vertical-align: middle;
    margin-right: 6px;
    flex-shrink: 0;
  }
</style>



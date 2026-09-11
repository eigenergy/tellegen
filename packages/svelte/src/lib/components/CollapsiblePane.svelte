<script lang="ts">
  interface Props {
    id: string;
    title: string;
    summary?: string;
    class?: string;
    children: import("svelte").Snippet;
  }

  let {
    id,
    title,
    summary = "",
    class: className = "",
    children,
  }: Props = $props();
  let open = $state(true);
</script>

<section
  class={`section-pane ${className}`}
  class:collapsed={!open}
  data-pane={id}
>
  <button
    type="button"
    class="section-head"
    aria-expanded={open}
    aria-controls={`${id}-pane-body`}
    onclick={() => (open = !open)}
  >
    <span class="disclosure" aria-hidden="true">&#9662;</span>
    <span class="head-copy">
      <strong>{title}</strong>
      {#if summary}<span class="mono">{summary}</span>{/if}
    </span>
  </button>
  {#if open}
    <div class="section-body" id={`${id}-pane-body`}>
      {@render children()}
    </div>
  {/if}
</section>

<style>
  .section-pane {
    flex: 0 0 auto;
    overflow: hidden;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 3px;
    box-shadow: var(--elev-2);
    backdrop-filter: blur(6px);
  }

  .section-head {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 46px;
    padding: 8px 12px;
    border: 0;
    background: transparent;
    color: var(--ink);
    text-align: left;
    cursor: pointer;
  }

  .section-head:hover {
    background: var(--accent-soft);
  }

  .section-head:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: -2px;
  }

  .disclosure {
    flex: 0 0 12px;
    color: var(--text-accent);
    font-size: 15px;
    line-height: 1;
    transition: transform var(--dur-fast) var(--ease-out);
  }

  .collapsed .disclosure {
    transform: rotate(-90deg);
  }

  .head-copy {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    flex-direction: column;
    gap: 1px;
  }

  .head-copy strong {
    font-size: 13px;
    font-weight: 600;
  }

  .head-copy span {
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 9.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .section-body {
    padding: 12px 16px 14px 32px;
    border-top: 1px solid var(--line);
  }

  @media (max-width: 760px) {
    .section-pane {
      border-width: 0 0 1px;
      border-radius: 0;
      box-shadow: none;
      backdrop-filter: none;
    }

    .section-head {
      min-height: 44px;
      padding-inline: 4px 8px;
    }

    .section-body {
      padding: 10px 8px 14px 24px;
    }
  }
</style>

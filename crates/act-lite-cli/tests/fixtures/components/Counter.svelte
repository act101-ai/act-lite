<script context="module" lang="ts">
  export const prerender = true;
</script>

<p>Zähler</p><script lang="ts">export let start: number = 0;
  let count = start;
  $: doubled = count * 2;
  function increment(step: number) {
    if (step > 0 && count < 10) {
      count += step;
    }
  }
</script>

<div class="counter">
  <button on:click={() => increment(1)}>+</button>
  {#if count > 5}
    <span>{count} {doubled}</span>
  {/if}
  {#snippet row(n: number)}<i>{n}</i>{/snippet}
</div>

<style>
  .counter { color: red; }
</style>

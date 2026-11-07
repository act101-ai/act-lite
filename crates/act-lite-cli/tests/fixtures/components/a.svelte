<script lang="ts">
  import { format } from "./format";

  let message = format("hello");
</script>

{#if message}
  <p class="greeting">{message}</p>
{/if}

<script lang="ts">
  import "../app.css";
  // Tactical HUD type stack: Chakra Petch (display labels), IBM Plex Sans
  // (body), IBM Plex Mono (values / telemetry). Latin subsets only.
  import "@fontsource/chakra-petch/latin-500.css";
  import "@fontsource/chakra-petch/latin-600.css";
  import "@fontsource/chakra-petch/latin-700.css";
  import "@fontsource/ibm-plex-sans/latin-400.css";
  import "@fontsource/ibm-plex-sans/latin-500.css";
  import "@fontsource/ibm-plex-sans/latin-600.css";
  import "@fontsource/ibm-plex-mono/latin-400.css";
  import "@fontsource/ibm-plex-mono/latin-500.css";
  import "@fontsource/ibm-plex-mono/latin-600.css";

  let { children } = $props();
</script>

{@render children()}

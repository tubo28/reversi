import { defineConfig } from "vite";

export default defineConfig({
  // Relative asset paths so the built site can be hosted under any subpath.
  base: "./",
  build: {
    outDir: "dist",
    // Keep the output easy to inspect / deploy as plain static files.
    emptyOutDir: true,
  },
});

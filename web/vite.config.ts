import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  // Relative asset paths so the built site can be hosted under any subpath.
  base: "./",
  plugins: [react()],
  build: {
    outDir: "dist",
    // Keep the output easy to inspect / deploy as plain static files.
    emptyOutDir: true,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
});

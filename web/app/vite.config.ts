import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The built site sits at the root of the CloudFront distribution, next to
// calculator/ (the WASM cost engine), which the app loads at runtime.
export default defineConfig({
  plugins: [react()],
  base: "./",
  build: { outDir: "dist", emptyOutDir: true },
  server: { port: 5173 },
});

import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Windowsでfork workerのIPC起動待ちが続くため、分離されたworker threadで実行する。
    pool: "threads",
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.ts"],
    restoreMocks: true,
  },
});
